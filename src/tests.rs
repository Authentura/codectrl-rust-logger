#![allow(dead_code)]
#![cfg(test)]

#[test]
fn log() { log_layer_2(); }

#[test]
fn log_if() { log_if_layer_2() }

#[test]
fn log_when_env() { log_when_env_layer_2() }

// normal log
fn log_layer_2() { log_layer_3(); }

fn log_layer_3() { log_final_layer(); }

fn log_final_layer() {
    let a = || crate::Logger::log("Hello", Some(2), None, None, None);
    let _ = a();
}

// log_if
fn log_if_gen_random() -> bool { rand::random() }

fn log_if_layer_2() { log_if_layer_3() }

fn log_if_layer_3() { log_if_final_layer() }

fn log_if_final_layer() {
    let some_variable = true;

    let _ = crate::Logger::log_if(
        log_if_gen_random,
        "Hello, conditional",
        Some(2),
        None,
        None,
        None,
    );
    let _ =
        crate::Logger::log_if(|| true, "Hello, conditional 2", Some(2), None, None, None);

    let _ = crate::Logger::boxed_log_if(
        Box::new(move || some_variable),
        "Hello, conditional 3",
        Some(2),
        None,
        None,
        None,
    );
}

// log_when_env
fn log_when_env_layer_2() { log_when_env_layer_3() }

fn log_when_env_layer_3() { log_when_env_final_layer() }

fn log_when_env_final_layer() {
    let _ = crate::Logger::log_when_env("Hello, world env", Some(2), None, None, None);
}
