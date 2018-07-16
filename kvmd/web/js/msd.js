var msd = new function() {
	this.loadInitialState = function() {
		var http = tools.makeRequest("GET", "/kvmd/msd", function() {
			if (http.readyState === 4) {
				if (http.status === 200) {
					msd.setState(JSON.parse(http.responseText).result);
				} else {
					setTimeout(msd.loadInitialState, 1000);
				}
			}
		});
	};

	this.setState = function(state) {
		if (state.connected_to == "server") {
			cls = "led-on";
		} else if (state.busy) {
			cls = "led-msd-writing";
		} else {
			cls = "led-off";
		}
		$("msd-led").className = cls;
	};
};
