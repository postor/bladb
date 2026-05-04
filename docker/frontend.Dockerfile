FROM node:22-bookworm-slim AS builder

WORKDIR /workspace

ENV PNPM_HOME=/pnpm
ENV PATH=$PNPM_HOME:$PATH

RUN corepack enable

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml tsconfig.base.json ./
COPY packages ./packages
COPY apps/examples ./apps/examples

ARG APP_DIR
ARG VITE_BLADB_URL=/api

ENV VITE_BLADB_URL=$VITE_BLADB_URL

RUN pnpm install --frozen-lockfile
RUN pnpm --dir "$APP_DIR" build

FROM nginx:1.27-alpine

ARG APP_DIR

COPY docker/nginx.examples.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /workspace/${APP_DIR}/dist /usr/share/nginx/html

EXPOSE 80
