function main() {
	if (checkBrowser()) {
		tools.setOnClick($("login-button"), __login);
		document.onkeyup = function(event) {
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
			}
			__setDisabled(false);
			$("passwd-input").focus();
			$("passwd-input").select();
		}
	}, body, "application/x-www-form-urlencoded");
	http.send();
	__setDisabled(true);
}

function __setDisabled(disabled) {
	$("user-input").disabled = disabled;
	$("passwd-input").disabled = disabled;
	$("login-button").disabled = disabled;
}
