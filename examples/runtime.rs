use simplest_async_runtime::{sleep::sleep, Runtime};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut runtime = Runtime::new();
    runtime.block_on(real_main());
    Ok(())
}

async fn real_main() {
    println!("Hello World");
    println!("Sleeping 5 seconds...");
    sleep(Duration::from_secs(5)).await;
    println!("Done!");
}
