#[cfg(test)]
mod tests;

use backtrace::Backtrace;
use codectrl_protobuf_bindings::{
    data::{BacktraceData, Log},
    logs_service::{LoggerClient, RequestResult, RequestStatus},
};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    env,
    error::Error,
    fmt::Debug,
    fs,
    fs::File,
    io::{prelude::*, BufReader},
};
use tokio::runtime::{Handle, Runtime};
use tonic::Request;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Warning {
    CompiledWithoutDebugInfo,
}

impl ToString for Warning {
    fn to_string(&self) -> String {
        match self {
            Self::CompiledWithoutDebugInfo =>
                "File was compiled without debug info, meaning information was lost",
        }
        .into()
    }
}

fn create_log<T: Debug>(message: T, surround: Option<u32>, offset: Option<u32>) -> Log {
    let offset = offset.unwrap_or_default();

    let mut log = Log {
        uuid: "".to_string(),
        stack: Vec::new(),
        line_number: 0,
        file_name: String::new(),
        code_snippet: BTreeMap::new(),
        message: format!("{:#?}", &message),
        message_type: std::any::type_name::<T>().to_string(),
        address: String::new(),
        warnings: Vec::new(),
        language: "Rust".into(),
    };

    #[cfg(not(debug_assertions))]
    eprintln!(
        "Unfortunately, using this function without debug_assertions enabled will \
         produce limited information. The stack trace, file path and line number will \
         be missing from the final message that is sent to the server. Please consider \
         guarding this function using #[cfg(debug_assertions)] so that this message \
         does not re-appear."
    );

    #[cfg(not(debug_assertions))]
    log.warnings
        .push(Warning::CompiledWithoutDebugInfo.to_string());

    let surround = surround.unwrap_or(3);

    Logger::get_stack_trace(&mut log);

    if let Some(last) = log.stack.last() {
        log.line_number = last.line_number + offset;
        log.code_snippet =
            Logger::get_code_snippet(&last.file_path, log.line_number, surround);

        log.file_name = last.file_path.clone();
    }

    log
}

pub struct LogBatch<'a> {
    logger: Logger<'a>,
    log_batch: VecDeque<Log>,
    tokio_runtime: Option<&'a Handle>,
    host: &'static str,
    port: &'static str,
    surround: u32,
}

impl<'a> LogBatch<'a> {
    fn new(logger: Logger<'a>) -> Self {
        Self {
            logger,
            log_batch: VecDeque::new(),
            tokio_runtime: None,
            host: "127.0.0.1",
            port: "3002",
            surround: 3,
        }
    }

    pub fn host(mut self, host: &'static str) -> Self {
        self.host = host;
        self
    }

    pub fn port(mut self, port: &'static str) -> Self {
        self.port = port;
        self
    }

    pub fn tokio_runtime(mut self, rt: &'a Handle) -> Self {
        self.tokio_runtime = Some(rt);
        self
    }

    pub fn surround(mut self, surround: u32) -> Self {
        self.surround = surround;
        self
    }

    pub fn add_log<T: Debug>(mut self, message: T, surround: Option<u32>) -> Self {
        let surround = Some(surround.unwrap_or(self.surround));

        self.log_batch.push_back(create_log(
            message,
            surround,
            Some(self.log_batch.len() as u32 + 1),
        ));

        self
    }

    pub fn add_log_if<T: Debug>(
        mut self,
        condition: fn() -> bool,
        message: T,
        surround: Option<u32>,
    ) -> Self {
        let surround = Some(surround.unwrap_or(self.surround));

        if condition() {
            self.log_batch.push_back(create_log(
                message,
                surround,
                Some(self.log_batch.len() as u32 + 1),
            ));
        }

        self
    }

    pub fn add_boxed_log_if<T: Debug>(
        mut self,
        condition: Box<dyn FnOnce() -> bool>,
        message: T,
        surround: Option<u32>,
    ) -> Self {
        let surround = Some(surround.unwrap_or(self.surround));

        if condition() {
            self.log_batch.push_back(create_log(
                message,
                surround,
                Some(self.log_batch.len() as u32 + 1),
            ));
        }

        self
    }

    pub fn add_log_when_env<T: Debug>(
        mut self,
        message: T,
        surround: Option<u32>,
    ) -> Self {
        let surround = Some(surround.unwrap_or(self.surround));

        if env::var("CODECTRL_DEBUG").ok().is_some() {
            self.log_batch.push_back(create_log(
                message,
                surround,
                Some(self.log_batch.len() as u32 + 1),
            ));
        } else {
            #[cfg(debug_assertions)]
            println!("add_log_when_env not called: envvar CODECTRL_DEBUG not present");
        }

        self
    }

    pub fn build(mut self) -> Logger<'a> {
        self.logger = Logger {
            log_batch: self.log_batch,
            batch_host: self.host,
            batch_port: self.port,
            batch_tokio_runtime: self.tokio_runtime,
        };

        self.logger
    }
}

#[derive(Debug, Clone, Default)]
pub struct Logger<'a> {
    log_batch: VecDeque<Log>,
    batch_host: &'static str,
    batch_port: &'static str,
    batch_tokio_runtime: Option<&'a Handle>,
}

impl<'a> Logger<'a> {
    pub fn start_batch() -> LogBatch<'a> { LogBatch::new(Self::default()) }

    pub fn send_batch(&mut self) -> Result<(), Box<dyn Error>> {
        if self.log_batch.is_empty() {
            return Err("Can't send batch: Log batch is empty".into());
        }

        let mut ret = Ok(());

        async fn send_batch(
            host: &str,
            port: &str,
            logs: &mut VecDeque<Log>,
        ) -> Result<(), Box<dyn Error>> {
            let mut log_client =
                LoggerClient::connect(format!("http://{host}:{port}")).await?;

            let request = Request::new(stream::iter(logs.clone()));
            let response = log_client.send_logs(request).await?;

            match response.into_inner() {
                RequestResult { status, .. }
                    if status == RequestStatus::Confirmed.into() =>
                    Ok(()),
                RequestResult { message, status }
                    if status == RequestStatus::Error.into() =>
                    Err(message.into()),
                RequestResult { .. } => unreachable!(),
            }
        }

        if let Some(handle) = self.batch_tokio_runtime {
            handle.block_on(async {
                ret = send_batch(self.batch_host, self.batch_port, &mut self.log_batch)
                    .await;
            });
        } else {
            let rt = Runtime::new()?;

            rt.block_on(async {
                ret = send_batch(self.batch_host, self.batch_port, &mut self.log_batch)
                    .await;
            })
        }

        Ok(())
    }

    /// The main log function that is called from Rust code.
    ///
    /// This function will print a warning to stderr if this crate is compiled
    /// with debug_assertions disabled as it will produce a much less
    /// informative log for codeCTRL.
    ///
    /// If given a pre-existing tokio runtime, it _will_ block the executor
    /// while it waits for the log to complete.
    pub fn log<T: Debug>(
        message: T,
        surround: Option<u32>,
        host: Option<&str>,
        port: Option<&str>,
        tokio_runtime: Option<&Handle>,
    ) -> Result<(), Box<dyn Error>> {
        let host = host.unwrap_or("127.0.0.1");
        let port = port.unwrap_or("3002");

        let mut log = create_log(message, surround, None);

        let mut ret = Ok(());

        if let Some(handle) = tokio_runtime {
            handle.block_on(async {
                ret = Self::_log(&mut log, host, port).await;
            });
        } else {
            let rt = Runtime::new()?;

            rt.block_on(async {
                ret = Self::_log(&mut log, host, port).await;
            })
        }

        ret
    }

    /// A log function that takes a closure and only logs out if that function
    /// returns `true`. Essentially a conditional wrapper over
    /// [`Self::log`]. See [`Self::boxed_log_if`] for a variation that
    /// allows for closures that take can take from values in scope.
    ///
    /// If given a pre-existing tokio runtime, it _will_ block the executor
    /// while it waits for the log to complete.
    pub fn log_if<T: Debug>(
        condition: fn() -> bool,
        message: T,
        surround: Option<u32>,
        host: Option<&str>,
        port: Option<&str>,
        tokio_runtime: Option<&Handle>,
    ) -> Result<bool, Box<dyn Error>> {
        if condition() {
            Self::log(message, surround, host, port, tokio_runtime)?;
            return Ok(true);
        }

        Ok(false)
    }

    /// A log function, similar to [`Self::log_if`] that takes a boxed closure
    /// or function that can take in parameters from the outer scope.
    ///
    /// If given a pre-existing tokio runtime, it _will_ block the executor
    /// while it waits for the log to complete.
    pub fn boxed_log_if<T: Debug>(
        condition: Box<dyn FnOnce() -> bool>,
        message: T,
        surround: Option<u32>,
        host: Option<&str>,
        port: Option<&str>,
        tokio_runtime: Option<&Handle>,
    ) -> Result<bool, Box<dyn Error>> {
        if condition() {
            Self::log(message, surround, host, port, tokio_runtime)?;
            return Ok(true);
        }

        Ok(false)
    }

    /// A log function, similar to [`Self::log_if`] and [`Self::boxed_log_if`],
    /// that only takes effect if the environment variable `CODECTRL_DEBUG`
    /// is present or not.
    ///
    /// If given a pre-existing tokio runtime, it _will_ block the executor
    /// while it waits for the log to complete.
    pub fn log_when_env<T: Debug>(
        message: T,
        surround: Option<u32>,
        host: Option<&str>,
        port: Option<&str>,
        tokio_runtime: Option<&Handle>,
    ) -> Result<bool, Box<dyn Error>> {
        if env::var("CODECTRL_DEBUG").ok().is_some() {
            Self::log(message, surround, host, port, tokio_runtime)?;
            Ok(true)
        } else {
            #[cfg(debug_assertions)]
            println!("log_when_env not called: envvar CODECTRL_DEBUG not present");

            Ok(false)
        }
    }

    // We have a non-async wrapper over _log so that we can log from non-async
    // scopes.
    //
    // TODO: Provide a direct wrapper so that async environments do not need to call
    // a non-async wrapper, just for that to call an async wrapper.
    async fn _log(log: &mut Log, host: &str, port: &str) -> Result<(), Box<dyn Error>> {
        let mut log_client =
            LoggerClient::connect(format!("http://{host}:{port}")).await?;

        let request = Request::new(log.clone());
        let response = log_client.send_log(request).await?;

        match response.into_inner() {
            RequestResult { status, .. } if status == RequestStatus::Confirmed.into() =>
                Ok(()),
            RequestResult { message, status }
                if status == RequestStatus::Error.into() =>
                Err(message.into()),
            RequestResult { .. } => unreachable!(),
        }
    }

    fn get_stack_trace(log: &mut Log) {
        let backtrace = Backtrace::new();

        for frame in backtrace.frames() {
            backtrace::resolve(frame.ip(), |symbol| {
                let name = if let Some(symbol) = symbol.name() {
                    let mut symbol = symbol.to_string();
                    let mut split = symbol.split("::").collect::<Vec<&str>>();

                    if split.len() > 1 {
                        split.remove(split.len() - 1);
                    }

                    symbol = split.join("::");

                    symbol
                } else {
                    "".into()
                };

                if let (Some(file_name), Some(line_number), Some(column_number)) =
                    (symbol.filename(), symbol.lineno(), symbol.colno())
                {
                    let file_path: String = if let Ok(path) = fs::canonicalize(file_name)
                    {
                        path.as_os_str().to_str().unwrap().to_string()
                    } else {
                        file_name.as_os_str().to_str().unwrap().to_string()
                    };

                    if !(name.contains("Logger::")
                        || name.contains("LogBatch::")
                        || name.ends_with("create_log")
                        || file_path.starts_with("/rustc/"))
                        && file_path.contains(".rs")
                    {
                        let code = Self::get_code(&file_path, line_number);

                        log.stack.insert(
                            0,
                            BacktraceData {
                                name,
                                file_path,
                                line_number,
                                column_number,
                                code,
                            },
                        );
                    }
                }
            });
        }
    }

    fn get_code(file_path: &str, line_number: u32) -> String {
        let mut code = String::new();

        let file = File::open(file_path).unwrap_or_else(|_| {
            panic!("Unexpected error: could not open file: {}", file_path)
        });

        let reader = BufReader::new(file);

        if let Some(Ok(line)) = reader.lines().nth(line_number.saturating_sub(1) as usize)
        {
            code = line.trim().to_string();
        }

        code
    }

    fn get_code_snippet(
        file_path: &str,
        line_number: u32,
        surround: u32,
    ) -> BTreeMap<u32, String> {
        let file = File::open(file_path).unwrap_or_else(|_| {
            panic!("Unexpected error: could not open file: {}", file_path)
        });

        let offset = line_number.saturating_sub(surround);
        let reader = BufReader::new(file);

        let lines: BTreeMap<u32, String> = reader
            .lines()
            .enumerate()
            .filter(|(_, line)| line.is_ok())
            .map(|(n, line)| ((n + 1) as u32, line.unwrap()))
            .collect();

        let end = line_number.saturating_add(surround);

        lines
            .range(offset..=end)
            .map(|(key, value)| (*key, value.clone()))
            .collect()
    }
}
