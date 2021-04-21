use actix_web::dev::{Service, Transform};
use actix_web::{dev::ServiceRequest, dev::ServiceResponse, Error};
use futures::future::{ok, Ready};
use futures::Future;
use std::io::{Error as IoError, ErrorKind};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

pub struct ProcessStatus {
    pub running: bool,
}

pub struct ExclusiveLocker {
    pub working: Arc<Mutex<ProcessStatus>>,
}

impl Default for ExclusiveLocker {
    fn default() -> Self {
        ExclusiveLocker {
            working: Arc::new(Mutex::new(ProcessStatus { running: false })),
        }
    }
}

// Middleware factory is `Transform` trait from actix-service crate
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S> for ExclusiveLocker
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = ExclusiveLockerMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(ExclusiveLockerMiddleware {
            service,
            working: self.working.clone(),
        })
    }
}

pub struct ExclusiveLockerMiddleware<S> {
    service: S,
    pub working: Arc<Mutex<ProcessStatus>>,
}

impl<S, B> Service for ExclusiveLockerMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        if req.path() == "/diary" && req.method() == "POST" {
            let arc: Arc<Mutex<ProcessStatus>> = Arc::clone(&self.working);
            // 無理やり実行を停止している
            if (*arc.as_ref().lock().unwrap()).running {
                Box::pin(async { Err(Error::from(IoError::new(ErrorKind::ConnectionRefused, "task is already running"))) })
            } else {
                // 問題ない場合はProcessStatusを変更する
                *arc.as_ref().lock().unwrap() = ProcessStatus { running: true };
                let arc2: Arc<Mutex<ProcessStatus>> = Arc::clone(&self.working);
                let fut = self.service.call(req);
                Box::pin(async move {
                    let res = fut.await?;
                    log::info!("end thread");
                    // 実行が終わったらfalseに戻す
                    *arc2.as_ref().lock().unwrap() = ProcessStatus { running: false };
                    Ok(res)
                })
            }
        } else {
            let fut = self.service.call(req);
            Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            })
        }
    }
}
