use super::{
    auth::InMemoryAuthService, config::LocalGatewayConfig, flash_sale::FlashSaleModule,
    iot::IotModule, AppApiHandler, AppApiRequest, AppError, GatewayRuntimeConfig,
};
use crate::{
    route_prepared_request, AuthContext, ExecutionContext, Gateway, GatewayError, ModuleRegistry,
    ModuleRuntime, RuntimeRegistry,
};
use bladb_core::protocol::{ErrorCode, GatewayRequest};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone)]
pub struct LocalGatewayApp {
    gateways: Vec<GatewayRuntime>,
    modules: RuntimeRegistry,
    app_apis: Vec<Arc<dyn AppApiHandler>>,
    auth_service: Arc<InMemoryAuthService>,
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
        auth_service: Arc<InMemoryAuthService>,
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
            auth_service,
        })
    }

    pub fn with_standard_modules(
        runtime_configs: Vec<GatewayRuntimeConfig>,
        auth_service: Arc<InMemoryAuthService>,
    ) -> Result<Self, String> {
        let flash_sale = Arc::new(FlashSaleModule::new());
        let iot = Arc::new(IotModule::new());
        let modules: Vec<Arc<dyn ModuleRuntime>> = vec![flash_sale.clone(), iot.clone()];
        let app_apis: Vec<Arc<dyn AppApiHandler>> = vec![flash_sale.clone(), iot];

        Self::from_parts(
            runtime_configs,
            RuntimeRegistry::new(modules),
            app_apis,
            auth_service,
        )
    }

    pub fn from_local_config(config: LocalGatewayConfig) -> Result<Self, String> {
        let auth_service = Arc::new(InMemoryAuthService::from_user_configs(config.auth_users));
        let mut runtimes: Vec<Arc<dyn ModuleRuntime>> = vec![];
        let mut app_apis: Vec<Arc<dyn AppApiHandler>> = vec![];

        if let Some(flash_sale_config) = config.modules.flash_sale {
            let module = Arc::new(FlashSaleModule::from_config(flash_sale_config));
            runtimes.push(module.clone());
            app_apis.push(module);
        }

        if let Some(iot_config) = config.modules.iot {
            let module = Arc::new(IotModule::from_config(iot_config));
            runtimes.push(module.clone());
            app_apis.push(module);
        }

        if runtimes.is_empty() {
            return Err("local gateway config must enable at least one module runtime".into());
        }

        Self::from_parts(
            config.runtimes,
            RuntimeRegistry::new(runtimes),
            app_apis,
            auth_service,
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
        let context = self.resolve_execution(&request, bearer_token)?;
        self.modules.execute(&context).map_err(AppError::from)
    }

    pub fn inspect_request_for_token(
        &self,
        request: GatewayRequest,
        bearer_token: Option<&str>,
    ) -> Result<Value, AppError> {
        let context = self.resolve_execution(&request, bearer_token)?;
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
        let session = self.auth_service.login(app, email, password)?;
        Ok(session.to_public_json())
    }

    pub fn register(
        &self,
        app: &str,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<Value, AppError> {
        let session = self
            .auth_service
            .register(app, email, password, display_name)?;
        Ok(session.to_public_json())
    }

    pub fn me(&self, bearer_token: &str) -> Result<Value, AppError> {
        let session = self.auth_service.session_from_bearer(bearer_token)?;
        Ok(session.to_public_json())
    }

    pub fn handle_app_api(
        &self,
        method: &str,
        path: &str,
        bearer_token: Option<&str>,
        body: Option<Value>,
    ) -> Result<Option<Value>, AppError> {
        let session = match bearer_token {
            Some(token) => Some(self.auth_service.session_from_bearer(token)?),
            None => None,
        };
        let Some(handler) = self
            .app_apis
            .iter()
            .find(|handler| handler.can_handle(method, path))
        else {
            return Ok(None);
        };

        handler
            .handle(AppApiRequest {
                method: method.into(),
                path: path.into(),
                body,
                session,
            })
            .map(Some)
    }

    fn resolve_execution(
        &self,
        request: &GatewayRequest,
        bearer_token: Option<&str>,
    ) -> Result<ExecutionContext, AppError> {
        let mut last_error: Option<GatewayError> = None;
        let session = match bearer_token {
            Some(token) => Some(self.auth_service.session_from_bearer(token)?),
            None => None,
        };

        for runtime in &self.gateways {
            if let Some(session) = &session {
                if session.user.app != runtime.name {
                    continue;
                }
            }

            let auth = session
                .as_ref()
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
}
