var wm;

function main() {
	if (checkBrowser()) {
		wm = new WindowManager();

		tools.setOnClick($("login-button"), __login);
		$("user-input").onkeyup = $("passwd-input").onkeyup = function(event) {
			if (event.code == "Enter") {
				event.preventDefault();
				__login();
			}
		};

		$("user-input").focus();
	}
}

function __login() {
	var user = $("user-input").value;
	var passwd = $("passwd-input").value;
	var body = `user=${encodeURIComponent(user)}&passwd=${encodeURIComponent(passwd)}`;

	var http = tools.makeRequest("POST", "/kvmd/auth/login", function() {
		if (http.readyState === 4) {
			if (http.status === 200) {
				document.location.href = "/";
			} else if (http.status === 403) {
				wm.error("Invalid username or password").then(__tryAgain);
			} else {
				wm.error("Login error:<br>", http.responseText).then(__tryAgain);
			}
		}
	}, body, "application/x-www-form-urlencoded");
	__setDisabled(true);
}

function __setDisabled(disabled) {
	$("user-input").disabled = disabled;
	$("passwd-input").disabled = disabled;
	$("login-button").disabled = disabled;
}

function __tryAgain() {
	__setDisabled(false);
	$("passwd-input").focus();
	$("passwd-input").select();
}
