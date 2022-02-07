#![allow(dead_code)]
#![cfg(feature = "full")]

#[test]
fn log() { log_layer_2(); }

fn log_layer_2() { log_layer_3(); }

fn log_layer_3() { log_final_layer(); }

fn log_final_layer() {
    let a = || crate::Log::log("Hello", Some(2), None, None);
    let _ = a();
}

#[test]
fn log_if_gen_random() -> bool {
    use rand::prelude::*;

    rand::random()
}

fn log_if() { log_if_layer_2() }

fn log_if_layer_2() { log_if_layer_3() }

fn log_if_layer_3() { log_if_final_layer() }

fn log_if_final_layer() {
    crate::Log::log_if(
        log_if_gen_random(),
        "Hello, conditional",
        Some(2),
        None,
        None,
    );
    // let mut rng =
}
