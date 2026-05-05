FROM node:22-bookworm-slim

WORKDIR /app

COPY apps/examples/ros2-backend/package.json ./
COPY apps/examples/ros2-backend/server.mjs ./

EXPOSE 8080

CMD ["node", "server.mjs"]
