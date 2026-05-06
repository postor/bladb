import {
  TENANT_ID,
  UID,
  createBrowserAppModule,
  createClient,
  type GatewaySession
} from "@bladb/client";

const viteEnv = (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env;

export const BLADB_URL = viteEnv?.VITE_BLADB_URL ?? "http://localhost:8787";
export const BLOG_APP = "blog";
export const BLOG_TOKEN_KEY = "bladb.blog-rust-user.token";
export const BLOG_SESSION_KEY = "bladb.blog-rust-user.session";

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
  published?: boolean;
}

const blogGuestDb = createClient({
  baseUrl: BLADB_URL,
  appAuth: "optional",
  executeAuth: "optional",
  sessionAppName: BLOG_APP
});

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
export const blogDb = blogModule.db;
export const blogUser = blogModule.user;

export async function listPublishedPosts() {
  return await blogGuestDb.app(BLOG_APP).get<BlogPost[]>("posts");
}

export async function readPublishedPost(slug: string) {
  return await blogGuestDb.app(BLOG_APP).get<BlogPost>(`posts/${slug}`);
}

export async function listMyPosts() {
  return await blogDb.app(BLOG_APP).get<BlogPost[]>("me/posts");
}

export async function createPost(session: BlogSession, input: BlogComposerInput) {
  return await blogDb.app(BLOG_APP).post<BlogPost>("me/posts", {
    title: input.title,
    slug: slugify(input.title),
    summary: input.summary,
    body: input.body,
    published: input.published ?? true,
    authorName: session.user.displayName
  });
}

export async function updateMyPost(postId: string, input: BlogComposerInput) {
  return await blogDb.app(BLOG_APP).patch<BlogPost>(`me/posts/${postId}`, {
    title: input.title,
    slug: slugify(input.title),
    summary: input.summary,
    body: input.body,
    published: input.published ?? true
  });
}

export async function deleteMyPost(postId: string) {
  return await blogDb.app(BLOG_APP).delete<{ deleted: boolean; id: string; slug: string }>(
    `me/posts/${postId}`
  );
}

export async function attemptHackUpdate(victimPostId: string) {
  return await blogDb.mongo("posts").updateOne<BlogPost>(
    {
      id: victimPostId,
      tenantId: TENANT_ID,
      authorUid: UID
    },
    {
      title: "Hacked title",
      summary: "This should never succeed",
      body: "Unauthorized overwrite attempt",
      slug: "hacked-title",
      published: true
    },
    {
      policy: "blog.posts.update-mine"
    }
  );
}
