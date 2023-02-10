use codectrl::{Logger, LoggerError};

fn main() -> anyhow::Result<()> {
    fn inner() -> Result<(), LoggerError> {
        Logger::log("Hello, world!", None, None, None, None)
    }

    inner()?;

    Ok(())
}
