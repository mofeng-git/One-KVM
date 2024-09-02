worker_processes 4;

# error_log /tmp/kvmd-nginx.error.log;
error_log stderr;

include /usr/share/kvmd/extras/*/nginx.ctx-main.conf;

events {
	worker_connections 1024;
	use epoll;
	multi_accept on;
}

http {
	types_hash_max_size 4096;
	server_names_hash_bucket_size 128;

	access_log off;

	include /etc/kvmd/nginx/mime-types.conf;
	default_type application/octet-stream;
	charset utf-8;

	sendfile on;
	tcp_nodelay on;
	tcp_nopush on;
	keepalive_timeout 10;
	client_max_body_size 4k;

	client_body_temp_path	/tmp/kvmd-nginx/client_body_temp;
	fastcgi_temp_path		/tmp/kvmd-nginx/fastcgi_temp;
	proxy_temp_path			/tmp/kvmd-nginx/proxy_temp;
	scgi_temp_path			/tmp/kvmd-nginx/scgi_temp;
	uwsgi_temp_path			/tmp/kvmd-nginx/uwsgi_temp;

	include /etc/kvmd/nginx/kvmd.ctx-http.conf;
	include /usr/share/kvmd/extras/*/nginx.ctx-http.conf;

	% if https_enabled:

	server {
		listen ${http_port};
		% if ipv6_enabled:
		listen [::]:${http_port};
		% endif
		include /etc/kvmd/nginx/certbot.ctx-server.conf;
		location / {
			% if https_port == 443:
			return 301 https://$host$request_uri;
			% else:
			return 301 https://$host:${https_port}$request_uri;
			% endif
		}
	}

	server {
		listen ${https_port} ssl http2;
		% if ipv6_enabled:
		listen [::]:${https_port} ssl http2;
		% endif
		include /etc/kvmd/nginx/ssl.conf;
		include /etc/kvmd/nginx/kvmd.ctx-server.conf;
		include /usr/share/kvmd/extras/*/nginx.ctx-server.conf;
	}

	% else:

	server {
		listen ${http_port};
		% if ipv6_enabled:
		listen [::]:${http_port};
		% endif
		include /etc/kvmd/nginx/certbot.ctx-server.conf;
		include /etc/kvmd/nginx/kvmd.ctx-server.conf;
		include /usr/share/kvmd/extras/*/nginx.ctx-server.conf;
	}

	% endif
}
