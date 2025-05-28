// I will never appear in the output of your site, don't worry. I'm here because you're running weaving serve
(() => {
	const ws = new WebSocket("ws://{SERVE_ADDRESS}/ws");
	ws.addEventListener("message", function(event) {
		console.log("Message from server (direct WS):", event.data);
		if (event.data === "reload") {
			console.log("Reloading page (direct WS fallback)...");
			window.location.reload();
		}
	})
	ws.addEventListener("open", () => console.log("Socket connected to dev server"));
	ws.addEventListener("close", () => console.log("Socket closed"));
	ws.addEventListener("error", (err) => console.error("WebSocket error:", err));
})()
