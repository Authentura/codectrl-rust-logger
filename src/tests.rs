#![allow(dead_code)]
#![cfg(test)]

use crate::Logger;
use std::{thread::sleep, time::Duration};

#[test]
fn log() { log_layer_2(); }

#[test]
fn log_if() { log_if_layer_2() }

#[test]
fn log_when_env() { log_when_env_layer_2() }

#[test]
fn log_batch() { log_batch_layer_2() }

// normal log
fn log_layer_2() { log_layer_3(); }

fn log_layer_3() { log_final_layer(); }

fn log_final_layer() {
    let a = || Logger::log("Hello", Some(2), None, None, None);
    if let Err(e) = a() {
        panic!("{e}");
    }
}

// log_if
fn log_if_gen_random() -> bool { rand::random() }

fn log_if_layer_2() { log_if_layer_3() }

fn log_if_layer_3() { log_if_final_layer() }

fn log_if_final_layer() {
    let some_variable = true;

    if let Err(e) = Logger::log_if(
        log_if_gen_random,
        "Hello, conditional",
        Some(2),
        None,
        None,
        None,
    ) {
        panic!("{e}");
    }

    if let Err(e) =
        Logger::log_if(|| true, "Hello, conditional 2", Some(2), None, None, None)
    {
        panic!("{e}");
    }

    if let Err(e) = Logger::boxed_log_if(
        Box::new(move || some_variable),
        "Hello, conditional 3",
        Some(2),
        None,
        None,
        None,
    ) {
        panic!("{e}");
    }
}

// log_when_env
fn log_when_env_layer_2() { log_when_env_layer_3() }

fn log_when_env_layer_3() { log_when_env_final_layer() }

fn log_when_env_final_layer() {
    if let Err(e) = Logger::log_when_env("Hello, world env", Some(2), None, None, None) {
        panic!("{e}");
    }
}

fn log_batch_layer_2() { log_batch_layer_3() }

fn log_batch_layer_3() {
    use chrono::Utc;

    let mut logger = Logger::start_batch()
        .add_log(Utc::now(), None)
        .add_log("Batched hello", None)
        .add_log_if(|| true, "Batched hello conditional", None)
        .add_log_if(|| false, "This won't show", None)
        .add_log_when_env("Batched env", Some(2))
        .build();

    sleep(Duration::new(1, 0));

    if let Err(e) = logger.send_batch() {
        panic!("{e}");
    }
}
