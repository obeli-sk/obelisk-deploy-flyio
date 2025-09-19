mod generated;
use generated::any::{export, exports::obelisk_flyio::workflow::workflow::Guest};

struct Component;
export!(Component with_types_in generated::any);

impl Guest for Component {
    fn app_create(
        app_name: String,
        volume_config: generated::any::exports::obelisk_flyio::workflow::workflow::VolumeConfig,
        config: generated::any::exports::obelisk_flyio::workflow::workflow::ObeliskConfig,
    ) -> Result<
        Vec<generated::any::exports::obelisk_flyio::workflow::workflow::SecretKey>,
        generated::any::exports::obelisk_flyio::workflow::workflow::AppCreateError,
    > {
        todo!()
    }

    fn secret_list_keys(
        app_name: String,
    ) -> Vec<generated::any::exports::obelisk_flyio::workflow::workflow::SecretKey> {
        todo!()
    }

    fn serve(
        app_name: String,
    ) -> Result<(), generated::any::exports::obelisk_flyio::workflow::workflow::ServeError> {
        todo!()
    }

    fn app_delete(name: String) -> () {
        todo!()
    }
}
