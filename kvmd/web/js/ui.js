var ui = new function() {
	this.init = function() {
		Array.prototype.forEach.call(document.getElementsByClassName("ctl-item"), function(el_item) {
			el_item.onclick = function() { __toggleMenu(el_item); };
		});

		window.onclick = __windowClickHandler;

		Array.prototype.forEach.call(document.getElementsByClassName("window"), function(el_window) {
			var el_header = el_window.querySelector(".window-header");
			__makeWindowMovable(el_header, el_window);
		});
	};

	var __toggleMenu = function(el_a) {
		Array.prototype.forEach.call(document.getElementsByClassName("ctl-item"), function(el_item) {
			var el_menu = el_item.parentElement.querySelector(".ctl-dropdown-content");
			if (el_item === el_a && el_menu.style.display === "none") {
				el_menu.style.display = "block";
				el_item.setAttribute("style", "background-color: var(--bg-color-selected)");
			} else {
				el_menu.style.display = "none";
				el_item.setAttribute("style", "background-color: default");
			}
		});
	};

	var __windowClickHandler = function(event) {
		if (!event.target.matches(".ctl-item")) {
			for (el_item = event.target; el_item && el_item !== document; el_item = el_item.parentNode) {
				if (el_item.hasAttribute("data-force-hide-menu")) {
					break;
				}
				else if (el_item.hasAttribute("data-dont-hide-menu")) {
					return;
				}
			}
			__toggleMenu(null);
		}
	};

	var __makeWindowMovable = function(el_header, el_body) {
		var prev_x = 0;
		var prev_y = 0;

		function startMoving(event) {
			event = (event || window.event);
			event.preventDefault();
			prev_x = event.clientX;
			prev_y = event.clientY;
			document.onmousemove = doMoving;
			document.onmouseup = stopMoving;
		}

		function doMoving(event) {
			event = (event || window.event);
			event.preventDefault();
			x = prev_x - event.clientX;
			y = prev_y - event.clientY;
			prev_x = event.clientX;
			prev_y = event.clientY;
			el_body.style.top = (el_body.offsetTop - y) + "px";
			el_body.style.left = (el_body.offsetLeft - x) + "px";
		}

		function stopMoving() {
			document.onmousemove = null;
			document.onmouseup = null;
		}

		el_header.onmousedown = startMoving;
	};
};
