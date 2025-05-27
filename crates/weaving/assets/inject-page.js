if ('serviceWorker' in navigator) {
	console.log("Weaving: Service workers are supported, installing for live reload.")
	navigator.serviceWorker.register('/service-worker.js')
		.then(function(registration) {
			registration.update().then(() => {
				console.log('Service Worker registered with scope:', registration.scope);
			})
		}).catch(function(error) {
			console.error('Service Worker registration failed:', error);
		});

	navigator.serviceWorker.addEventListener('message', function(event) {
		console.log('Message from Service Worker:', event.data);
		if (event.data === 'reload') {
			console.log('Reloading page...');
			window.location.reload();
		}
	});
} else {
	console.warn('Weaving: Service Workers are not supported. Fallback to direct WebSocket.');
	const ws = new WebSocket("ws://localhost:8080/ws");
	ws.onmessage = function(event) {
		console.log("Message from server (direct WS):", event.data);
		if (event.data === "reload") {
			console.log("Reloading page (direct WS fallback)...");
			window.location.reload();
		}
	};
	ws.onopen = () => console.log("Direct WebSocket opened");
	ws.onclose = () => console.log("Direct WebSocket closed");
	ws.onerror = (err) => console.error("Direct WebSocket error:", err);
}

