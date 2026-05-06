use super::{AppApiHandler, AppApiRequest, AppError};
use crate::{ExecutionContext, ModuleRuntime, RuntimeError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{any::Any, sync::Mutex};

pub struct BlogModule {
    state: Mutex<BlogState>,
    allow_anonymous_app_access: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlogModuleConfig {
    #[serde(default)]
    pub posts: Vec<BlogPostConfig>,
    #[serde(default)]
    pub allow_anonymous_app_access: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlogPostConfig {
    pub id: String,
    pub tenant_id: String,
    pub author_uid: String,
    pub author_name: String,
    pub title: String,
    pub slug: String,
    pub summary: String,
    pub body: String,
    pub published: bool,
    pub created_at: String,
}

struct BlogState {
    posts: Vec<BlogPostConfig>,
    next_post_id: u64,
}

impl BlogModule {
    pub fn new() -> Self {
        Self::from_config(BlogModuleConfig {
            posts: vec![
                BlogPostConfig {
                    id: "post_0001".into(),
                    tenant_id: "tenant_blog".into(),
                    author_uid: "u_5001".into(),
                    author_name: "Blog Editor".into(),
                    title: "Welcome to the Bladb blog example".into(),
                    slug: "welcome-to-the-bladb-blog-example".into(),
                    summary: "A seeded article that demonstrates list and mine flows.".into(),
                    body: "This example shows how db.user and db.mongo can power a small content workflow.".into(),
                    published: true,
                    created_at: "2026-05-06T08:00:00Z".into(),
                },
                BlogPostConfig {
                    id: "post_0002".into(),
                    tenant_id: "tenant_blog".into(),
                    author_uid: "u_5001".into(),
                    author_name: "Blog Editor".into(),
                    title: "Drafting with tenant-aware storage".into(),
                    slug: "drafting-with-tenant-aware-storage".into(),
                    summary: "A second post to make the public list feel real.".into(),
                    body: "The local runtime keeps ownership, tenancy, and ordering inside the trusted path.".into(),
                    published: true,
                    created_at: "2026-05-06T09:30:00Z".into(),
                },
            ],
            allow_anonymous_app_access: true,
        })
    }

    pub fn from_config(config: BlogModuleConfig) -> Self {
        Self {
            state: Mutex::new(BlogState {
                next_post_id: config.posts.len() as u64 + 1,
                posts: config.posts,
            }),
            allow_anonymous_app_access: config.allow_anonymous_app_access,
        }
    }

    fn render_post(post: &BlogPostConfig) -> Value {
        json!({
            "id": post.id,
            "tenantId": post.tenant_id,
            "authorUid": post.author_uid,
            "authorName": post.author_name,
            "title": post.title,
            "slug": post.slug,
            "summary": post.summary,
            "body": post.body,
            "published": post.published,
            "createdAt": post.created_at
        })
    }

    fn filtered_posts(
        state: &BlogState,
        query: &Map<String, Value>,
        options: Option<&bladb_core::protocol::QueryOptions>,
    ) -> Value {
        let tenant_id = query.get("tenantId").and_then(Value::as_str);
        let author_uid = query.get("authorUid").and_then(Value::as_str);
        let published = query.get("published").and_then(Value::as_bool);
        let slug = query.get("slug").and_then(Value::as_str);

        let mut posts = state
            .posts
            .iter()
            .filter(|post| tenant_id.is_none_or(|value| post.tenant_id == value))
            .filter(|post| author_uid.is_none_or(|value| post.author_uid == value))
            .filter(|post| published.is_none_or(|value| post.published == value))
            .filter(|post| slug.is_none_or(|value| post.slug == value))
            .cloned()
            .collect::<Vec<_>>();

        posts.sort_by(|left, right| right.created_at.cmp(&left.created_at));

        let offset = options.and_then(|value| value.offset).unwrap_or(0) as usize;
        let limit = options.and_then(|value| value.limit).unwrap_or(posts.len() as u64) as usize;

        Value::Array(
            posts.into_iter()
                .skip(offset)
                .take(limit)
                .map(|post| Self::render_post(&post))
                .collect(),
        )
    }

    fn find_one_post(state: &BlogState, query: &Map<String, Value>) -> Result<Value, RuntimeError> {
        let tenant_id = query.get("tenantId").and_then(Value::as_str);
        let slug = query.get("slug").and_then(Value::as_str);

        let post = state
            .posts
            .iter()
            .find(|post| {
                tenant_id.is_none_or(|value| post.tenant_id == value)
                    && slug.is_none_or(|value| post.slug == value)
            })
            .ok_or_else(|| RuntimeError::not_found("blog post not found"))?;

        Ok(Self::render_post(post))
    }

    fn insert_post(
        state: &mut BlogState,
        document: &Map<String, Value>,
    ) -> Result<Value, RuntimeError> {
        let tenant_id = required_string(document, "tenantId")?;
        let author_uid = required_string(document, "authorUid")?;
        let author_name = required_string(document, "authorName")?;
        let title = required_string(document, "title")?;
        let slug = required_string(document, "slug")?;
        let summary = required_string(document, "summary")?;
        let body = required_string(document, "body")?;
        let published = document
            .get("published")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        let post = BlogPostConfig {
            id: format!("post_{:04}", state.next_post_id),
            tenant_id,
            author_uid,
            author_name,
            title,
            slug,
            summary,
            body,
            published,
            created_at: crate::local::now_label(),
        };
        state.next_post_id += 1;
        state.posts.insert(0, post.clone());

        Ok(Self::render_post(&post))
    }

    fn published_posts(&self, tenant_id: &str, limit: Option<u64>) -> Result<Value, AppError> {
        let state = self.state.lock().map_err(AppError::lock_runtime)?;
        let query = json!({
            "tenantId": tenant_id,
            "published": true
        });
        let query = query.as_object().expect("published query");
        let options = bladb_core::protocol::QueryOptions {
            limit,
            offset: Some(0),
        };

        Ok(Self::filtered_posts(&state, query, Some(&options)))
    }
}

impl ModuleRuntime for BlogModule {
    fn handles_cluster(&self, cluster: &str) -> bool {
        cluster.starts_with("blog.")
    }

    fn execute(&self, context: &ExecutionContext) -> Result<Value, RuntimeError> {
        let body = &context.routed.body;
        let collection = body
            .collection
            .as_deref()
            .ok_or_else(|| RuntimeError::invalid_request("collection is missing"))?;
        if collection != "posts" {
            return Err(RuntimeError::invalid_request(format!(
                "unsupported blog collection `{collection}`"
            )));
        }

        match (context.request.action.as_str(), context.request.kind.clone()) {
            ("find", bladb_core::protocol::RequestKind::Query) => {
                let state = self.state.lock().map_err(crate::local::AppError::lock_runtime)?;
                let query = body
                    .query
                    .as_ref()
                    .ok_or_else(|| RuntimeError::invalid_request("query is missing"))?;
                Ok(Self::filtered_posts(&state, query, body.options.as_ref()))
            }
            ("findOne", bladb_core::protocol::RequestKind::Query) => {
                let state = self.state.lock().map_err(crate::local::AppError::lock_runtime)?;
                let query = body
                    .query
                    .as_ref()
                    .ok_or_else(|| RuntimeError::invalid_request("query is missing"))?;
                Self::find_one_post(&state, query)
            }
            ("insertOne", bladb_core::protocol::RequestKind::Command) => {
                let mut state = self.state.lock().map_err(crate::local::AppError::lock_runtime)?;
                let document = body
                    .document
                    .as_ref()
                    .ok_or_else(|| RuntimeError::invalid_request("document is missing"))?;
                Self::insert_post(&mut state, document)
            }
            _ => Err(RuntimeError::internal(format!(
                "unsupported blog operation `{}`",
                context.request.action
            ))),
        }
    }
}

impl AppApiHandler for BlogModule {
    fn can_handle(&self, method: &str, path: &str) -> bool {
        method.eq_ignore_ascii_case("GET") && path == "/apps/blog/posts"
    }

    fn handle(&self, request: AppApiRequest) -> Result<Value, AppError> {
        if !request.method.eq_ignore_ascii_case("GET") || request.path != "/apps/blog/posts" {
            return Err(AppError::not_found("blog app route not found"));
        }

        let tenant_id = if let Some(session) = request.session {
            session.user.tenant_id
        } else if self.allow_anonymous_app_access {
            "tenant_blog".to_string()
        } else {
            return Err(AppError::unauthorized("missing bearer token"));
        };

        self.published_posts(&tenant_id, Some(20))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn required_string(document: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    document
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| RuntimeError::invalid_request(format!("{field} is missing")))
}

#[cfg(test)]
mod tests {
    use super::BlogModule;
    use crate::{
        runtime::ModuleRuntime, AuthContext, Authorization, ExecutionContext, RouteSelection,
        RoutedRequest,
    };
    use bladb_core::{
        cluster::ModuleCategory,
        protocol::{Engine, GatewayRequest, QueryOptions, RequestBody, RequestKind},
    };
    use serde_json::json;

    fn context(body: RequestBody, action: &str, kind: RequestKind) -> ExecutionContext {
        ExecutionContext {
            request: GatewayRequest {
                kind: kind.clone(),
                engine: Engine::Mongo,
                action: action.into(),
                meta: Default::default(),
                body: body.clone(),
            },
            auth: AuthContext {
                uid: Some("u_5001".into()),
                tenant_id: Some("tenant_blog".into()),
                roles: vec!["editor".into()],
                permission_version: Some("v1".into()),
            },
            routed: RoutedRequest {
                authorization: Authorization {
                    policy_name: format!("blog.posts.{action}"),
                },
                route: RouteSelection {
                    cluster: "blog.posts-mongo".into(),
                    category: ModuleCategory::Data,
                    runtime: "mongo".into(),
                    service: "bladb-module-blog".into(),
                    namespace: Some("bladb".into()),
                    route_key: Some("tenant_blog".into()),
                    shard: Some(1),
                    sticky: false,
                },
                body,
            },
        }
    }

    #[test]
    fn blog_module_lists_seeded_posts() {
        let module = BlogModule::new();
        let response = module
            .execute(&context(
                RequestBody {
                    collection: Some("posts".into()),
                    query: Some(
                        json!({
                            "tenantId": "tenant_blog",
                            "published": true
                        })
                        .as_object()
                        .expect("query map")
                        .clone(),
                    ),
                    options: Some(QueryOptions {
                        limit: Some(10),
                        offset: Some(0),
                    }),
                    ..Default::default()
                },
                "find",
                RequestKind::Query,
            ))
            .expect("list posts");

        let rows = response.as_array().expect("rows");
        assert!(!rows.is_empty());
        assert_eq!(rows[0]["tenantId"], "tenant_blog");
    }

    #[test]
    fn blog_module_inserts_and_reads_owned_post() {
        let module = BlogModule::new();
        let created = module
            .execute(&context(
                RequestBody {
                    collection: Some("posts".into()),
                    document: Some(
                        json!({
                            "tenantId": "tenant_blog",
                            "authorUid": "u_5001",
                            "authorName": "Blog Editor",
                            "title": "A fresh post",
                            "slug": "a-fresh-post",
                            "summary": "Testing insertOne",
                            "body": "This one is created during the unit test.",
                            "published": true
                        })
                        .as_object()
                        .expect("document map")
                        .clone(),
                    ),
                    ..Default::default()
                },
                "insertOne",
                RequestKind::Command,
            ))
            .expect("insert post");

        assert_eq!(created["slug"], "a-fresh-post");

        let mine = module
            .execute(&context(
                RequestBody {
                    collection: Some("posts".into()),
                    query: Some(
                        json!({
                            "tenantId": "tenant_blog",
                            "authorUid": "u_5001"
                        })
                        .as_object()
                        .expect("query map")
                        .clone(),
                    ),
                    ..Default::default()
                },
                "find",
                RequestKind::Query,
            ))
            .expect("list mine");

        let rows = mine.as_array().expect("mine rows");
        assert!(rows.iter().any(|row| row["slug"] == "a-fresh-post"));
    }
}
