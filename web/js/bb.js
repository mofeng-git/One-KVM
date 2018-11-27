function checkBrowser() {
	if (
		!window.navigator
		|| window.navigator.userAgent.indexOf("MSIE ") > 0
		|| window.navigator.userAgent.indexOf("Trident/") > 0
		|| window.navigator.userAgent.indexOf("Edge/") > 0
	) {
		var el_modal = document.createElement("div");
		el_modal.className = "modal";
		el_modal.style.visibility = "visible";
		el_modal.innerHTML = `
			<div class="modal-window">
				<div class="modal-content">
					Hello. You are using an incompatible or legacy browser.<br>
					Please use one of the following browsers:
					<hr>
					<ul>
						<li><a target="_blank" href="https://google.com/chrome">Google Chrome</a> <sup><i>recommended</i></sup></li>
						<li><a target="_blank" href="https://chromium.org/Home">Chromium</a> <sup><i>recommended</i></sup></li>
						<li><a target="_blank" href="https://mozilla.org/firefox">Mozilla Firefox</a></li>
						<li><a target="_blank" href="https://apple.com/safari">Apple Safari</a></li>
						<li><a target="_blank" href="https://opera.com">Opera</a></li>
						<li><a target="_blank" href="https://vivaldi.com">Vivaldi</a></li>
					</ul>
				</div>
			</div>
		`;
		document.body.appendChild(el_modal);
		return false;
	} else {
		return true;
	}
}
