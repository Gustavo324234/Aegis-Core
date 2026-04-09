fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = prost_build::Config::new();

    // Box large enum variants to satisfy clippy
    config.boxed(".ank.v1.TaskEvent.payload.status_update");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos_with_config(
            config,
            &["../../proto/kernel.proto", "../../proto/siren.proto"],
            &["../../proto"],
        )?;
    Ok(())
}
