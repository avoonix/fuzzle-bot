FROM node:24-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm install
COPY . .
RUN npm run build

FROM nginx:mainline-alpine-slim
COPY --from=builder /app/dist /usr/share/nginx/html

COPY default.conf.template /etc/nginx/templates/default.conf.template
