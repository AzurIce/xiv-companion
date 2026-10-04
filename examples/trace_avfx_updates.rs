//! Explicit-update creation probe. Input is a raw AVFX and a JSON array of
//! Framework deltas in seconds; stdout is the trace JSON.
//! cargo run --example trace_avfx_updates -- effect.avfx 0 -1 steps.json

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err(
            "usage: trace_avfx_updates FILE.avfx EMITTER_INDEX ROOT_LIFE STEPS.json".into(),
        );
    }
    let bytes = std::fs::read(&args[0])?;
    let file = xiv_companion_data::AvfxFile::parse(&bytes)?;
    let emitter = args[1].parse()?;
    let life = args[2].parse()?;
    let steps: Vec<f32> = serde_json::from_slice(&std::fs::read(&args[3])?)?;
    let runtime = xiv_companion_data::VfxRuntime::new(&file);
    let trace = runtime.trace_emitter_updates(emitter, life, &steps)?;
    serde_json::to_writer_pretty(std::io::stdout().lock(), &trace)?;
    Ok(())
}
