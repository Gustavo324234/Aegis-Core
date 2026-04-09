pub mod v1 {
    tonic::include_proto!("ank.v1");

    pub mod siren {
        tonic::include_proto!("ank.v1.siren");
    }
}
