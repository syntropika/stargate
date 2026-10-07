use rust::NativeRuntime;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
fn main() {
    let mut runtime = None;
    for line in io::stdin().lock().lines() {
        let output = (|| -> Result<Value, String> {
            let command: Value = serde_json::from_str(&line.map_err(|_| "input failed")?)
                .map_err(|_| "invalid command")?;
            let input = command["input"].to_string();
            match command["operation"].as_str().ok_or("missing operation")? {
                "create" => {
                    runtime = Some(NativeRuntime::create(&input)?);
                    Ok(json!({"ready":true}))
                }
                "handle" => serde_json::from_str(
                    &runtime.as_ref().ok_or("not initialized")?.handle(&input)?,
                )
                .map_err(|_| "encoding failed".into()),
                "authorize" => serde_json::from_str(
                    &runtime
                        .as_ref()
                        .ok_or("not initialized")?
                        .authorize(&input)?,
                )
                .map_err(|_| "encoding failed".into()),
                _ => Err("unknown operation".into()),
            }
        })()
        .unwrap_or_else(|error| json!({"error":error}));
        println!("{output}");
        io::stdout().flush().unwrap();
    }
}
