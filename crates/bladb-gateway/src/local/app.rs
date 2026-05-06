use super::{
    auth::AuthSession,
    auth::InMemoryUserConfig,
    blog::BlogModule,
    config::LocalGatewayConfig,
    flash_sale::FlashSaleModule,
    iot::{IotModule, IotSubscription},
    ros2::{Ros2Module, Ros2Subscription},
    user::{OfficialUserModule, SessionCookie},
    AppApiHandler, AppApiRequest, AppError, GatewayRuntimeConfig,
};
use crate::{
    route_prepared_request, AuthContext, ExecutionContext, Gateway, GatewayError, ModuleRegistry,
    ModuleRuntime, RuntimeRegistry,
};
use bladb_core::protocol::{ErrorCode, GatewayRequest};
use serde_json::{json, Value};
use std::{collections::HashSet, sync::Arc};

#[derive(Clone)]
pub struct GatewayHttpResponse<T> {
    pub data: T,
    pub session_cookie: Option<SessionCookie>,
}

#[derive(Clone)]
pub struct LocalGatewayApp {
    gateways: Vec<GatewayRuntime>,
    modules: RuntimeRegistry,
    app_apis: Vec<Arc<dyn AppApiHandler>>,
    user_module: OfficialUserModule,
    anonymous_apps: HashSet<String>,
}

#[derive(Clone)]
struct GatewayRuntime {
    name: String,
    gateway: Gateway,
    registry: ModuleRegistry,
    auth: AuthContext,
}

impl LocalGatewayApp {
    pub(crate) fn from_parts(
        runtime_configs: Vec<GatewayRuntimeConfig>,
        modules: RuntimeRegistry,
        app_apis: Vec<Arc<dyn AppApiHandler>>,
        user_module: OfficialUserModule,
        anonymous_apps: HashSet<String>,
    ) -> Result<Self, String> {
        let gateways = runtime_configs
            .iter()
            .map(|config| {
                let name = config.name.clone();
                Ok(GatewayRuntime {
                    name,
                    gateway: Gateway::from_yaml(&config.policy_yaml).map_err(|error| {
                        format!("failed to init {} gateway: {error}", config.name)
                    })?,
                    registry: ModuleRegistry::from_yaml(&config.topology_yaml).map_err(
                        |error| format!("failed to init {} topology: {error}", config.name),
                    )?,
                    auth: config.default_auth.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        Ok(Self {
            gateways,
            modules,
            app_apis,
            user_module,
            anonymous_apps,
        })
    }

    pub fn with_standard_modules(
        runtime_configs: Vec<GatewayRuntimeConfig>,
        user_module: OfficialUserModule,
    ) -> Result<Self, String> {
        let flash_sale = Arc::new(FlashSaleModule::new());
        let blog = Arc::new(BlogModule::new());
        let iot = Arc::new(IotModule::new());
        let ros2 = Arc::new(Ros2Module::new());
        let modules: Vec<Arc<dyn ModuleRuntime>> =
            vec![blog, flash_sale.clone(), iot.clone(), ros2.clone()];
        let app_apis: Vec<Arc<dyn AppApiHandler>> = vec![flash_sale.clone(), iot, ros2];
        let anonymous_apps = HashSet::from([
            "blog".to_string(),
            "flash-sale".to_string(),
            "iot-realtime".to_string(),
            "ros2-bridge".to_string(),
        ]);

        Self::from_parts(
            runtime_configs,
            RuntimeRegistry::new(modules),
            app_apis,
            user_module,
            anonymous_apps,
        )
    }

    pub fn with_standard_modules_and_seed_users(
        runtime_configs: Vec<GatewayRuntimeConfig>,
        seed_users: Vec<InMemoryUserConfig>,
    ) -> Result<Self, String> {
        let user_module = OfficialUserModule::from_config(None, seed_users)?;
        Self::with_standard_modules(runtime_configs, user_module)
    }

    pub fn from_local_config(config: LocalGatewayConfig) -> Result<Self, String> {
        let user_module =
            OfficialUserModule::from_config(config.official_users.clone(), config.auth_users)?;
        let mut runtimes: Vec<Arc<dyn ModuleRuntime>> = vec![];
        let mut app_apis: Vec<Arc<dyn AppApiHandler>> = vec![];
        let mut anonymous_apps = HashSet::new();

        if let Some(flash_sale_config) = config.modules.flash_sale {
            if flash_sale_config.allow_anonymous_app_access {
                anonymous_apps.insert("flash-sale".to_string());
            }
            let module = Arc::new(FlashSaleModule::from_config(flash_sale_config));
            runtimes.push(module.clone());
            app_apis.push(module);
        }

        if let Some(blog_config) = config.modules.blog {
            if blog_config.allow_anonymous_app_access {
                anonymous_apps.insert("blog".to_string());
            }
            let module = Arc::new(BlogModule::from_config(blog_config));
            runtimes.push(module.clone());
            app_apis.push(module);
        }

        if let Some(iot_config) = config.modules.iot {
            if iot_config.allow_anonymous_app_access {
                anonymous_apps.insert("iot-realtime".to_string());
            }
            let module = Arc::new(IotModule::from_config(iot_config));
            runtimes.push(module.clone());
            app_apis.push(module);
        }

        if let Some(ros2_config) = config.modules.ros2 {
            if ros2_config.allow_anonymous_app_access {
                anonymous_apps.insert("ros2-bridge".to_string());
            }
            let module = Arc::new(Ros2Module::from_config(ros2_config));
            runtimes.push(module.clone());
            app_apis.push(module);
        }

        if runtimes.is_empty() && !user_module.is_enabled() {
            return Err("local gateway config must enable at least one module runtime".into());
        }

        Self::from_parts(
            config.runtimes,
            RuntimeRegistry::new(runtimes),
            app_apis,
            user_module,
            anonymous_apps,
        )
    }

    pub fn handle_execute(&self, request: GatewayRequest) -> Result<Value, AppError> {
        let context = self.resolve_execution(&request, None)?;
        self.modules.execute(&context).map_err(AppError::from)
    }

    pub fn inspect_request(&self, request: GatewayRequest) -> Result<Value, AppError> {
        let context = self.resolve_execution(&request, None)?;
        Ok(json!({
            "tenantId": context.auth.tenant_id,
            "policy": context.policy_name(),
            "route": {
                "cluster": context.route().cluster,
                "category": context.route().category,
                "runtime": context.route().runtime,
                "service": context.route().service,
                "namespace": context.route().namespace,
                "routeKey": context.route().route_key,
                "shard": context.route().shard,
                "sticky": context.route().sticky
            },
            "body": context.routed.body
        }))
    }

    pub fn handle_execute_for_token(
        &self,
        request: GatewayRequest,
        bearer_token: Option<&str>,
    ) -> Result<Value, AppError> {
        let context = self.resolve_execution_with_bearer(&request, bearer_token)?;
        self.modules.execute(&context).map_err(AppError::from)
    }

    pub fn inspect_request_for_token(
        &self,
        request: GatewayRequest,
        bearer_token: Option<&str>,
    ) -> Result<Value, AppError> {
        let context = self.resolve_execution_with_bearer(&request, bearer_token)?;
        Ok(json!({
            "tenantId": context.auth.tenant_id,
            "policy": context.policy_name(),
            "route": {
                "cluster": context.route().cluster,
                "category": context.route().category,
                "runtime": context.route().runtime,
                "service": context.route().service,
                "namespace": context.route().namespace,
                "routeKey": context.route().route_key,
                "shard": context.route().shard,
                "sticky": context.route().sticky
            },
            "body": context.routed.body
        }))
    }

    pub fn topology_snapshot(&self) -> Value {
        Value::Array(
            self.gateways
                .iter()
                .map(|runtime| {
                    json!({
                        "gateway": runtime.name,
                        "auth": {
                            "uid": runtime.auth.uid,
                            "tenantId": runtime.auth.tenant_id,
                            "roles": runtime.auth.roles,
                            "permissionVersion": runtime.auth.permission_version,
                        },
                        "clusters": runtime.registry.clusters().iter().map(|cluster| {
                            json!({
                                "name": cluster.name,
                                "category": cluster.category,
                                "runtime": cluster.runtime,
                                "policies": cluster.policies,
                                "service": cluster.discovery.service,
                                "namespace": cluster.discovery.namespace,
                                "discovery": cluster.discovery.kind,
                                "routing": {
                                    "strategy": cluster.routing.strategy,
                                    "routeBy": cluster.routing.route_by,
                                    "sticky": cluster.routing.sticky,
                                },
                                "transport": {
                                    "protocol": cluster.transport.protocol,
                                    "subject": cluster.transport.subject,
                                    "queueGroup": cluster.transport.queue_group,
                                    "stream": cluster.transport.stream,
                                    "durable": cluster.transport.durable,
                                },
                                "routeBy": cluster.routing.route_by,
                                "sticky": cluster.routing.sticky,
                                "deployment": {
                                    "replicas": cluster.deployment.replicas,
                                    "minReadySeconds": cluster.deployment.min_ready_seconds,
                                    "rolling": {
                                        "maxUnavailable": cluster.deployment.rolling.max_unavailable,
                                        "maxSurge": cluster.deployment.rolling.max_surge,
                                    },
                                    "autoscale": cluster.deployment.autoscale,
                                },
                            })
                        }).collect::<Vec<_>>()
                    })
                })
                .collect(),
        )
    }

    pub fn login(&self, app: &str, email: &str, password: &str) -> Result<Value, AppError> {
        self.user_login(app, email, password)
    }

    pub fn register(
        &self,
        app: &str,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<Value, AppError> {
        self.user_register(app, email, password, display_name)
    }

    pub fn me(&self, bearer_token: &str) -> Result<Value, AppError> {
        self.user_me(bearer_token)
    }

    pub fn user_login(&self, app: &str, email: &str, password: &str) -> Result<Value, AppError> {
        self.user_login_http(app, email, password)
            .map(|response| response.data)
    }

    pub fn user_register(
        &self,
        app: &str,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<Value, AppError> {
        self.user_register_http(app, email, password, display_name)
            .map(|response| response.data)
    }

    pub fn user_me(&self, bearer_token: &str) -> Result<Value, AppError> {
        self.user_module.me(bearer_token)
    }

    pub fn user_logout(&self, bearer_token: &str) -> Result<Value, AppError> {
        self.user_module.logout(bearer_token)
    }

    pub fn user_login_http(
        &self,
        app: &str,
        email: &str,
        password: &str,
    ) -> Result<GatewayHttpResponse<Value>, AppError> {
        let session = self.user_module.login_session(app, email, password)?;
        Ok(GatewayHttpResponse {
            data: session.to_public_json(),
            session_cookie: Some(SessionCookie::from_session(&session)),
        })
    }

    pub fn user_register_http(
        &self,
        app: &str,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<GatewayHttpResponse<Value>, AppError> {
        let session = self
            .user_module
            .register_session(app, email, password, display_name)?;
        Ok(GatewayHttpResponse {
            data: session.to_public_json(),
            session_cookie: Some(SessionCookie::from_session(&session)),
        })
    }

    pub fn user_me_http(
        &self,
        app: Option<&str>,
        bearer_token: Option<&str>,
        cookie_token: Option<&str>,
    ) -> Result<GatewayHttpResponse<Value>, AppError> {
        let session = self
            .user_module
            .me_for_request(app, bearer_token, cookie_token)?;
        Ok(GatewayHttpResponse {
            data: session.to_public_json(),
            session_cookie: Some(SessionCookie::from_session(&session)),
        })
    }

    pub fn user_logout_http(
        &self,
        app: Option<&str>,
        bearer_token: Option<&str>,
        cookie_token: Option<&str>,
    ) -> Result<GatewayHttpResponse<Value>, AppError> {
        let session_cookie = self
            .user_module
            .logout_for_request(app, bearer_token, cookie_token)?;
        Ok(GatewayHttpResponse {
            data: json!({ "revoked": true }),
            session_cookie,
        })
    }

    pub fn handle_app_api(
        &self,
        method: &str,
        path: &str,
        bearer_token: Option<&str>,
        body: Option<Value>,
    ) -> Result<Option<Value>, AppError> {
        self.handle_app_api_http(method, path, bearer_token, None, body)
            .map(|response| response.data)
    }

    pub fn handle_app_api_http(
        &self,
        method: &str,
        path: &str,
        bearer_token: Option<&str>,
        cookie_token: Option<&str>,
        body: Option<Value>,
    ) -> Result<GatewayHttpResponse<Option<Value>>, AppError> {
        let Some(handler) = self
            .app_apis
            .iter()
            .find(|handler| handler.can_handle(method, path))
        else {
            return Ok(GatewayHttpResponse {
                data: None,
                session_cookie: None,
            });
        };
        let app_name = app_name_from_app_path(path);
        let session = match app_name.as_deref() {
            Some(app) => self.resolve_app_session(app, bearer_token, cookie_token)?,
            None => None,
        };
        let session_cookie = session.as_ref().map(SessionCookie::from_session);

        let data = handler
            .handle(AppApiRequest {
                method: method.into(),
                path: path.into(),
                body,
                session,
            })
            .map(Some)?;

        Ok(GatewayHttpResponse { data, session_cookie })
    }

    pub fn open_ros2_stream(
        &self,
        path: &str,
        bearer_token: Option<&str>,
        cookie_token: Option<&str>,
    ) -> Result<Option<Ros2Subscription>, AppError> {
        let session = match app_name_from_app_path(path) {
            Some(app) => self.resolve_app_session(&app, bearer_token, cookie_token)?,
            None => None,
        };
        self.find_ros2_stream(path, session.as_ref())
    }

    pub fn open_iot_stream(
        &self,
        path: &str,
        bearer_token: Option<&str>,
        cookie_token: Option<&str>,
    ) -> Result<Option<IotSubscription>, AppError> {
        let session = match app_name_from_app_path(path) {
            Some(app) => self.resolve_app_session(&app, bearer_token, cookie_token)?,
            None => None,
        };
        self.find_iot_stream(path, session.as_ref())
    }

    fn resolve_execution(
        &self,
        request: &GatewayRequest,
        bearer_token: Option<&str>,
    ) -> Result<ExecutionContext, AppError> {
        let session = match bearer_token {
            Some(token) => Some(self.user_module.session_from_bearer(token)?),
            None => None,
        };
        self.resolve_execution_with_session(request, session.as_ref())
    }

    fn resolve_execution_with_bearer(
        &self,
        request: &GatewayRequest,
        bearer_token: Option<&str>,
    ) -> Result<ExecutionContext, AppError> {
        let token = bearer_token.ok_or_else(|| AppError::unauthorized("missing bearer token"))?;
        let session = self.user_module.session_from_bearer(token)?;
        self.resolve_execution_with_session(request, Some(&session))
    }

    fn resolve_execution_with_session(
        &self,
        request: &GatewayRequest,
        session: Option<&AuthSession>,
    ) -> Result<ExecutionContext, AppError> {
        let mut last_error: Option<GatewayError> = None;
        for runtime in &self.gateways {
            if let Some(session) = session {
                if session.user.app != runtime.name {
                    continue;
                }
            }

            let auth = session
                .map(|session| session.user.auth_context())
                .unwrap_or_else(|| runtime.auth.clone());

            match runtime.gateway.prepare(request, &auth) {
                Ok(prepared) => {
                    let routed =
                        route_prepared_request(&runtime.registry, request, prepared, &auth)
                            .map_err(|error| {
                                AppError::internal(format!("failed to route request: {error}"))
                            })?;

                    return Ok(ExecutionContext {
                        request: request.clone(),
                        auth,
                        routed,
                    });
                }
                Err(error @ GatewayError::UnknownPolicy(_))
                | Err(error @ GatewayError::PolicyMismatch { .. })
                | Err(error @ GatewayError::NoPolicyMatch) => {
                    last_error = Some(error);
                }
                Err(error @ GatewayError::InvalidRequest(_)) => {
                    return Err(AppError::invalid_request(error.to_string()));
                }
                Err(error) => {
                    return Err(AppError::invalid_request(error.to_string()));
                }
            }
        }

        let message = last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "no gateway could authorize request".into());

        Err(AppError {
            status: 403,
            code: ErrorCode::PolicyDenied,
            message,
        })
    }

    fn resolve_app_session(
        &self,
        app: &str,
        bearer_token: Option<&str>,
        cookie_token: Option<&str>,
    ) -> Result<Option<AuthSession>, AppError> {
        if let Some(token) = bearer_token {
            return self.user_module.session_from_bearer(token).map(Some);
        }

        if let Some(cookie) = cookie_token {
            return self.user_module.session_from_cookie(app, cookie).map(Some);
        }

        if self.anonymous_apps.contains(app) {
            return self.user_module.ensure_anonymous_session(app).map(Some);
        }

        Ok(None)
    }

    fn find_ros2_stream(
        &self,
        path: &str,
        session: Option<&AuthSession>,
    ) -> Result<Option<Ros2Subscription>, AppError> {
        for handler in &self.app_apis {
            let Some(ros2) = handler.as_any().downcast_ref::<Ros2Module>() else {
                continue;
            };
            if let Some(subscription) = ros2.open_message_stream(session, path)? {
                return Ok(Some(subscription));
            }
        }

        Ok(None)
    }

    fn find_iot_stream(
        &self,
        path: &str,
        session: Option<&AuthSession>,
    ) -> Result<Option<IotSubscription>, AppError> {
        for handler in &self.app_apis {
            let Some(iot) = handler.as_any().downcast_ref::<IotModule>() else {
                continue;
            };
            if let Some(subscription) = iot.open_command_stream(session, path)? {
                return Ok(Some(subscription));
            }
        }

        Ok(None)
    }
}

fn app_name_from_app_path(path: &str) -> Option<String> {
    let trimmed = path.trim_start_matches('/');
    let mut segments = trimmed.split('/');
    if segments.next()? != "apps" {
        return None;
    }

    segments
        .next()
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
}
