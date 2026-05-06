use super::{AppApiHandler, AppApiRequest, AppError};
use crate::local::auth::AuthSession;
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
                    author_uid: "u_5002".into(),
                    author_name: "Guest Writer".into(),
                    title: "How another author appears in the plaza".into(),
                    slug: "how-another-author-appears-in-the-plaza".into(),
                    summary:
                        "A seeded second-author article so the homepage plaza is not single-owner."
                            .into(),
                    body:
                        "Readers should see more than one voice in the shared square, while edit rights still stay bound to the owning author."
                            .into(),
                    published: true,
                    created_at: "2026-05-06T10:15:00Z".into(),
                },
                BlogPostConfig {
                    id: "post_0003".into(),
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
        let limit = options
            .and_then(|value| value.limit)
            .unwrap_or(posts.len() as u64) as usize;

        Value::Array(
            posts
                .into_iter()
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

    fn update_post(
        state: &mut BlogState,
        post_id: &str,
        author_uid: &str,
        patch: &Map<String, Value>,
    ) -> Result<Value, RuntimeError> {
        let post = state
            .posts
            .iter_mut()
            .find(|post| post.id == post_id)
            .ok_or_else(|| RuntimeError::not_found("blog post not found"))?;

        if post.author_uid != author_uid {
            return Err(RuntimeError::forbidden(
                "cannot modify another author's article",
            ));
        }

        if let Some(title) = patch.get("title").and_then(Value::as_str) {
            post.title = title.to_string();
        }
        if let Some(slug) = patch.get("slug").and_then(Value::as_str) {
            post.slug = slug.to_string();
        }
        if let Some(summary) = patch.get("summary").and_then(Value::as_str) {
            post.summary = summary.to_string();
        }
        if let Some(body) = patch.get("body").and_then(Value::as_str) {
            post.body = body.to_string();
        }
        if let Some(published) = patch.get("published").and_then(Value::as_bool) {
            post.published = published;
        }

        Ok(Self::render_post(post))
    }

    fn delete_post(
        state: &mut BlogState,
        post_id: &str,
        author_uid: &str,
    ) -> Result<Value, RuntimeError> {
        let index = state
            .posts
            .iter()
            .position(|post| post.id == post_id)
            .ok_or_else(|| RuntimeError::not_found("blog post not found"))?;

        if state.posts[index].author_uid != author_uid {
            return Err(RuntimeError::forbidden(
                "cannot delete another author's article",
            ));
        }

        let removed = state.posts.remove(index);
        Ok(json!({
            "deleted": true,
            "id": removed.id,
            "slug": removed.slug
        }))
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

    fn post_by_slug(&self, tenant_id: &str, slug: &str) -> Result<Value, AppError> {
        let state = self.state.lock().map_err(AppError::lock_runtime)?;
        let query = json!({
            "tenantId": tenant_id,
            "slug": slug
        });
        let query = query.as_object().expect("slug query");
        Self::find_one_post(&state, query).map_err(AppError::from)
    }

    fn posts_for_author(&self, tenant_id: &str, author_uid: &str) -> Result<Value, AppError> {
        let state = self.state.lock().map_err(AppError::lock_runtime)?;
        let query = json!({
            "tenantId": tenant_id,
            "authorUid": author_uid
        });
        let query = query.as_object().expect("author query");
        let options = bladb_core::protocol::QueryOptions {
            limit: Some(50),
            offset: Some(0),
        };

        Ok(Self::filtered_posts(&state, query, Some(&options)))
    }

    fn create_post_for_author(
        &self,
        session: &AuthSession,
        body: Option<&Value>,
    ) -> Result<Value, AppError> {
        let payload = body
            .and_then(Value::as_object)
            .ok_or_else(|| AppError::invalid_request("post body is required"))?;
        let mut document = Map::new();
        document.insert("tenantId".into(), Value::String(session.user.tenant_id.clone()));
        document.insert("authorUid".into(), Value::String(session.user.uid.clone()));
        document.insert(
            "authorName".into(),
            Value::String(session.user.display_name.clone()),
        );
        document.insert(
            "title".into(),
            Value::String(required_app_string(payload, "title")?),
        );
        document.insert(
            "slug".into(),
            Value::String(required_app_string(payload, "slug")?),
        );
        document.insert(
            "summary".into(),
            Value::String(required_app_string(payload, "summary")?),
        );
        document.insert(
            "body".into(),
            Value::String(required_app_string(payload, "body")?),
        );
        document.insert(
            "published".into(),
            Value::Bool(payload.get("published").and_then(Value::as_bool).unwrap_or(true)),
        );

        let mut state = self.state.lock().map_err(AppError::lock_runtime)?;
        Self::insert_post(&mut state, &document).map_err(AppError::from)
    }

    fn update_post_for_author(
        &self,
        session: &AuthSession,
        post_id: &str,
        body: Option<&Value>,
    ) -> Result<Value, AppError> {
        let payload = body
            .and_then(Value::as_object)
            .ok_or_else(|| AppError::invalid_request("post patch is required"))?;
        let mut state = self.state.lock().map_err(AppError::lock_runtime)?;
        Self::update_post(&mut state, post_id, &session.user.uid, payload).map_err(AppError::from)
    }

    fn delete_post_for_author(
        &self,
        session: &AuthSession,
        post_id: &str,
    ) -> Result<Value, AppError> {
        let mut state = self.state.lock().map_err(AppError::lock_runtime)?;
        Self::delete_post(&mut state, post_id, &session.user.uid).map_err(AppError::from)
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

        match (
            context.request.action.as_str(),
            context.request.kind.clone(),
        ) {
            ("find", bladb_core::protocol::RequestKind::Query) => {
                let state = self
                    .state
                    .lock()
                    .map_err(crate::local::AppError::lock_runtime)?;
                let query = body
                    .query
                    .as_ref()
                    .ok_or_else(|| RuntimeError::invalid_request("query is missing"))?;
                Ok(Self::filtered_posts(&state, query, body.options.as_ref()))
            }
            ("findOne", bladb_core::protocol::RequestKind::Query) => {
                let state = self
                    .state
                    .lock()
                    .map_err(crate::local::AppError::lock_runtime)?;
                let query = body
                    .query
                    .as_ref()
                    .ok_or_else(|| RuntimeError::invalid_request("query is missing"))?;
                Self::find_one_post(&state, query)
            }
            ("insertOne", bladb_core::protocol::RequestKind::Command) => {
                let mut state = self
                    .state
                    .lock()
                    .map_err(crate::local::AppError::lock_runtime)?;
                let document = body
                    .document
                    .as_ref()
                    .ok_or_else(|| RuntimeError::invalid_request("document is missing"))?;
                Self::insert_post(&mut state, document)
            }
            ("updateOne", bladb_core::protocol::RequestKind::Command) => {
                let mut state = self
                    .state
                    .lock()
                    .map_err(crate::local::AppError::lock_runtime)?;
                let query = body
                    .query
                    .as_ref()
                    .ok_or_else(|| RuntimeError::invalid_request("query is missing"))?;
                let document = body
                    .document
                    .as_ref()
                    .ok_or_else(|| RuntimeError::invalid_request("document is missing"))?;
                let post_id = required_string(query, "id")?;
                let author_uid = required_string(query, "authorUid")?;
                Self::update_post(&mut state, &post_id, &author_uid, document)
            }
            ("deleteOne", bladb_core::protocol::RequestKind::Command) => {
                let mut state = self
                    .state
                    .lock()
                    .map_err(crate::local::AppError::lock_runtime)?;
                let query = body
                    .query
                    .as_ref()
                    .ok_or_else(|| RuntimeError::invalid_request("query is missing"))?;
                let post_id = required_string(query, "id")?;
                let author_uid = required_string(query, "authorUid")?;
                Self::delete_post(&mut state, &post_id, &author_uid)
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
        if method.eq_ignore_ascii_case("GET") && path == "/apps/blog/posts" {
            return true;
        }

        if method.eq_ignore_ascii_case("GET")
            && path.starts_with("/apps/blog/posts/")
            && path.split('/').count() == 5
        {
            return true;
        }

        if path == "/apps/blog/me/posts" {
            return method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("POST");
        }

        if path.starts_with("/apps/blog/me/posts/") && path.split('/').count() == 6 {
            return method.eq_ignore_ascii_case("PATCH") || method.eq_ignore_ascii_case("DELETE");
        }

        false
    }

    fn handle(&self, request: AppApiRequest) -> Result<Value, AppError> {
        let AppApiRequest {
            method,
            path,
            body,
            session,
        } = request;

        if method.eq_ignore_ascii_case("GET") && path == "/apps/blog/posts" {
            let tenant_id = if let Some(session) = session.as_ref() {
                session.user.tenant_id.clone()
            } else if self.allow_anonymous_app_access {
                "tenant_blog".to_string()
            } else {
                return Err(AppError::unauthorized("missing bearer token"));
            };

            return self.published_posts(&tenant_id, Some(20));
        }

        if method.eq_ignore_ascii_case("GET")
            && path.starts_with("/apps/blog/posts/")
        {
            let tenant_id = if let Some(session) = session.as_ref() {
                session.user.tenant_id.clone()
            } else if self.allow_anonymous_app_access {
                "tenant_blog".to_string()
            } else {
                return Err(AppError::unauthorized("missing bearer token"));
            };
            let slug = path.trim_start_matches("/apps/blog/posts/").trim();
            return self.post_by_slug(&tenant_id, slug);
        }

        if path == "/apps/blog/me/posts" {
            let session = session
                .as_ref()
                .ok_or_else(|| AppError::unauthorized("missing bearer token"))?;
            if method.eq_ignore_ascii_case("GET") {
                return self.posts_for_author(&session.user.tenant_id, &session.user.uid);
            }
            if method.eq_ignore_ascii_case("POST") {
                return self.create_post_for_author(session, body.as_ref());
            }
        }

        if path.starts_with("/apps/blog/me/posts/") {
            let session = session
                .as_ref()
                .ok_or_else(|| AppError::unauthorized("missing bearer token"))?;
            let post_id = path.trim_start_matches("/apps/blog/me/posts/").trim();
            if method.eq_ignore_ascii_case("PATCH") {
                return self.update_post_for_author(session, post_id, body.as_ref());
            }
            if method.eq_ignore_ascii_case("DELETE") {
                return self.delete_post_for_author(session, post_id);
            }
        }

        Err(AppError::not_found("blog app route not found"))
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

fn required_app_string(document: &Map<String, Value>, field: &str) -> Result<String, AppError> {
    document
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| AppError::invalid_request(format!("{field} is missing")))
}

#[cfg(test)]
mod tests {
    use super::BlogModule;
    use crate::{
        local::{AppApiHandler, AppApiRequest, InMemoryAuthService},
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
    fn blog_module_seeds_multiple_authors_for_public_square() {
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
                        limit: Some(20),
                        offset: Some(0),
                    }),
                    ..Default::default()
                },
                "find",
                RequestKind::Query,
            ))
            .expect("list posts");

        let rows = response.as_array().expect("rows");
        let authors = rows
            .iter()
            .filter_map(|row| row["authorUid"].as_str())
            .collect::<std::collections::BTreeSet<_>>();

        assert!(
            authors.len() >= 2,
            "expected public square seeds to contain multiple authors, got {authors:?}"
        );
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

    #[test]
    fn blog_app_routes_restrict_updates_to_article_owner() {
        let module = BlogModule::new();
        let auth = InMemoryAuthService::from_user_configs(vec![
            crate::local::InMemoryUserConfig {
                app: "blog".into(),
                uid: "u_5001".into(),
                tenant_id: "tenant_blog".into(),
                email: "editor@blog.demo".into(),
                password: "demo123".into(),
                display_name: "Blog Editor".into(),
                roles: vec!["editor".into()],
            },
            crate::local::InMemoryUserConfig {
                app: "blog".into(),
                uid: "u_5002".into(),
                tenant_id: "tenant_blog".into(),
                email: "guest@blog.demo".into(),
                password: "demo123".into(),
                display_name: "Guest Writer".into(),
                roles: vec!["editor".into()],
            },
        ]);

        let owner = auth
            .login("blog", "editor@blog.demo", "demo123")
            .expect("owner session");
        let attacker = auth
            .login("blog", "guest@blog.demo", "demo123")
            .expect("attacker session");

        let created = module
            .handle(AppApiRequest {
                method: "POST".into(),
                path: "/apps/blog/me/posts".into(),
                body: Some(json!({
                    "title": "Owner post",
                    "slug": "owner-post",
                    "summary": "owner summary",
                    "body": "owner body",
                    "published": true
                })),
                session: Some(owner.clone()),
            })
            .expect("create owned article");
        let post_id = created["id"].as_str().expect("post id");

        let updated = module
            .handle(AppApiRequest {
                method: "PATCH".into(),
                path: format!("/apps/blog/me/posts/{post_id}"),
                body: Some(json!({
                    "title": "Owner post updated",
                    "slug": "owner-post-updated",
                    "summary": "owner summary updated",
                    "body": "owner body updated",
                    "published": true
                })),
                session: Some(owner.clone()),
            })
            .expect("owner update");
        assert_eq!(updated["title"], "Owner post updated");

        let patch_error = module
            .handle(AppApiRequest {
                method: "PATCH".into(),
                path: format!("/apps/blog/me/posts/{post_id}"),
                body: Some(json!({
                    "title": "Hacked title"
                })),
                session: Some(attacker),
            })
            .expect_err("foreign update should fail");
        assert_eq!(patch_error.status, 403);
        assert!(patch_error.message.contains("another author's article"));

        let delete_error = module
            .handle(AppApiRequest {
                method: "DELETE".into(),
                path: format!("/apps/blog/me/posts/{post_id}"),
                body: None,
                session: Some(
                    auth.login("blog", "guest@blog.demo", "demo123")
                        .expect("fresh attacker session"),
                ),
            })
            .expect_err("foreign delete should fail");
        assert_eq!(delete_error.status, 403);
        assert!(delete_error.message.contains("another author's article"));

        let deleted = module
            .handle(AppApiRequest {
                method: "DELETE".into(),
                path: format!("/apps/blog/me/posts/{post_id}"),
                body: None,
                session: Some(owner),
            })
            .expect("owner delete");
        assert_eq!(deleted["deleted"], true);
    }
}
