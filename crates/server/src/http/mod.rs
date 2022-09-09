use actix_web::dev::{AppService, HttpServiceFactory};

pub struct HttpApi {

}

impl HttpApi {
    pub fn new() -> HttpApi {
        Self {}
    }
}

impl HttpServiceFactory for HttpApi {
    fn register(self, _config: &mut AppService) {

    }
}
