# Use Nginx as the base image
FROM nginx:alpine

# Remove default nginx index page
RUN rm -rf /usr/share/nginx/html/*

# Copy the frontend files to the Nginx web directory
COPY ./frontend /usr/share/nginx/html

# Copy custom Nginx configuration
COPY ./frontend/nginx.conf /etc/nginx/nginx.conf

# Expose port 80
EXPOSE 80

# Start Nginx
CMD ["nginx", "-g", "daemon off;"]