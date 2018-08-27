var modal = new function() {
	this.error = (...args) => __modalDialog("Error", args.join(" "), true, false);
	this.confirm = (...args) => __modalDialog("Question", args.join(" "), true, true);

	var __modalDialog = function(header, text, ok, cancel) {
		var el_modal = document.createElement("div");
		el_modal.className = "modal";
		el_modal.style.visibility = "visible";
		el_modal.setAttribute("data-dont-hide-menu", "");

		var el_window = document.createElement("div");
		el_window.className = "modal-window";
		el_window.setAttribute("tabindex", "-1");
		el_modal.appendChild(el_window);

		var el_header = document.createElement("div");
		el_header.className = "modal-header";
		el_header.innerHTML = header;
		el_window.appendChild(el_header);

		var el_content = document.createElement("div");
		el_content.className = "modal-content";
		el_content.innerHTML = text;
		el_window.appendChild(el_content);

		var promise = null;
		if (ok || cancel) {
			promise = new Promise(function(resolve) {
				var el_buttons = document.createElement("div");
				el_buttons.className = "modal-buttons";
				el_window.appendChild(el_buttons);

				if (cancel) {
					var el_cancel_button = document.createElement("button");
					el_cancel_button.innerHTML = "Cancel";
					el_cancel_button.setAttribute("data-force-hide-menu", "");
					el_cancel_button.onclick = function() {
						el_modal.outerHTML = "";
						resolve(false);
					};
					el_buttons.appendChild(el_cancel_button);
				}
				if (ok) {
					var el_ok_button = document.createElement("button");
					el_ok_button.innerHTML = "OK";
					el_ok_button.setAttribute("data-force-hide-menu", "");
					el_ok_button.onclick = function() {
						el_modal.outerHTML = "";
						resolve(true);
					};
					el_buttons.appendChild(el_ok_button);
				}
				if (ok && cancel) {
					el_ok_button.className = "row50";
					el_cancel_button.className = "row50";
				}
			});
		}

		document.body.appendChild(el_modal);
		el_window.focus();

		return promise;
	};
};
