//! PHP/Laravel runtime provider: package primitive, runtime policy, managed
//! Nginx routing and PHP-FPM pools (phase 14).

pub mod nginx;
pub mod php_fpm;
pub mod policy;
pub mod worker;

pub use nginx::{ManagedSite, NginxError, NginxManager, SiteKind, render_site};
pub use php_fpm::{FpmError, FpmManager, ManagedPool, render_pool};
pub use policy::{RuntimeError, RuntimePolicy, validate_hostname};
pub use worker::{
    ScheduleUnit, WorkerError, WorkerManager, WorkerUnit, render_schedule_service,
    render_schedule_timer, render_worker,
};
