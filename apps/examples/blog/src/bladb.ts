import {
  TENANT_ID,
  UID,
  appGet,
  createBrowserAppModule,
  createClient,
  createTypedAppClient,
  type GatewaySession
} from "@bladb/client";

const viteEnv = (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env;

export const BLADB_URL = viteEnv?.VITE_BLADB_URL ?? "http://localhost:8787";
export const BLOG_APP = "blog";
export const BLOG_TOKEN_KEY = "bladb.blog.token";
export const BLOG_SESSION_KEY = "bladb.blog.session";

export type BlogSession = GatewaySession;

export interface BlogPost {
  id: string;
  tenantId: string;
  authorUid: string;
  authorName: string;
  title: string;
  slug: string;
  summary: string;
  body: string;
  published: boolean;
  createdAt: string;
}

export interface BlogComposerInput {
  title: string;
  summary: string;
  body: string;
}

const blogPublicRoutes = {
  publishedPosts: appGet<BlogPost[]>("posts")
};

export function slugify(title: string) {
  return title
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
}

export function createBlogModule() {
  return createBrowserAppModule({
    baseUrl: BLADB_URL,
    appName: BLOG_APP,
    tokenKey: BLOG_TOKEN_KEY,
    sessionKey: BLOG_SESSION_KEY,
    routes: {}
  });
}

export const blogModule = createBlogModule();
export const blogGuestDb = createClient({
  baseUrl: BLADB_URL,
  appAuth: "optional",
  executeAuth: "optional"
});
export const blogDb = blogModule.db;
export const blogUser = blogModule.user;
export const blogPublicApi = createTypedAppClient(blogGuestDb.app("blog"), blogPublicRoutes);

export async function listPublishedPosts() {
  return await blogPublicApi.publishedPosts();
}

export async function listMyPosts() {
  return await blogDb.mongo("posts").find<BlogPost[]>(
    {
      tenantId: TENANT_ID,
      authorUid: UID
    },
    {
      limit: 20
    },
    {
      policy: "blog.posts.list-mine"
    }
  );
}

export async function createPost(session: BlogSession, input: BlogComposerInput) {
  const slug = slugify(input.title);
  return await blogDb.mongo("posts").insertOne<BlogPost>(
    {
      tenantId: TENANT_ID,
      authorUid: UID,
      authorName: session.user.displayName,
      title: input.title,
      slug,
      summary: input.summary,
      body: input.body,
      published: true
    },
    {
      policy: "blog.posts.create"
    }
  );
}
