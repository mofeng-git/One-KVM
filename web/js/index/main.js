function main() {
	__setAppText();
	__loadKvmdInfo();
}

function __setAppText() {
	var url = window.location.href;
	$("app-text").innerHTML = `
		<span class="code-comment"># On Linux using Chromium/Chrome via any terminal:<br>
		$</span> \`which chromium 2>/dev/null || which chrome 2>/dev/null\` --app="${url}"<br>
		<br>
		<span class="code-comment"># On MacOS using Terminal application:<br>
		$</span> /Applications/Google&bsol; Chrome.app/Contents/MacOS/Google&bsol; Chrome --app="${url}"<br>
		<br>
		<span class="code-comment"># On Windows via cmd.exe:<br>
		C:&bsol;&gt;</span> start chrome --app="${url}"
	`;
}

function __loadKvmdInfo() {
	var http = tools.makeRequest("GET", "/kvmd/info", function() {
		if (http.readyState === 4) {
			if (http.status === 200) {
				var info = JSON.parse(http.responseText).result;

				var apps = Object.values(info.extras).sort(function(a, b) {
					if (a["place"] < b["place"]) {
						return -1;
					} else if (a["place"] > b["place"]) {
						return 1;
					} else {
						return 0;
					}
				});
				apps.forEach(function(app) {
					$("apps").innerHTML += `
						<li>
							<div class="app">
								<a href="${app.path}">
									<div>
										<img class="svg-gray" src="${app.icon}">
										${app.name}
									</div>
								</a>
							</div>
						</li>
					`;
				});

				if (info.meta && info.meta.server && info.meta.server.host) {
					$("kvmd-meta-server-host").innerHTML = info.meta.server.host;
					document.title = "Pi-KVM Index: " + info.meta.server.host;
				} else {
					$("kvmd-meta-server-host").innerHTML = "";
					document.title = "Pi-KVM Index";
				}
			} else {
				setTimeout(__loadKvmdInfo, 1000);
			}
		}
	});
}
