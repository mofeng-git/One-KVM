var ui = new function() {
	this.toggleMenu = function(el_a) {
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

	this.windowClickHandler = function(event) {
		if (!event.target.matches(".ctl-item")) {
			for (el_item = event.target; el_item && el_item !== document; el_item = el_item.parentNode) {
				if (el_item.hasAttribute("data-force-hide-menu")) {
					break;
				}
				else if (el_item.hasAttribute("data-dont-hide-menu")) {
					return;
				}
			}
			ui.toggleMenu(null);
		}
	};
};
