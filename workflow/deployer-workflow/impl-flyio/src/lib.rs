mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}
use generated::{export, exports::obelisk_flyio::workflow::workflow::Guest};

struct Component;
export!(Component with_types_in generated);

impl Guest for Component {
    fn app_create(
        app_name: String,
        volume_config: generated::exports::obelisk_flyio::workflow::workflow::VolumeConfig,
        config: generated::exports::obelisk_flyio::workflow::workflow::ObeliskConfig,
    ) -> Result<
        Vec<generated::exports::obelisk_flyio::workflow::workflow::SecretKey>,
        generated::exports::obelisk_flyio::workflow::workflow::AppCreateError,
    > {
        todo!()
    }

    fn secret_list_keys(
        app_name: String,
    ) -> Vec<generated::exports::obelisk_flyio::workflow::workflow::SecretKey> {
        todo!()
    }

    fn serve(
        app_name: String,
    ) -> Result<(), generated::exports::obelisk_flyio::workflow::workflow::ServeError> {
        todo!()
    }

    fn app_delete(name: String) -> () {
        todo!()
    }
}
