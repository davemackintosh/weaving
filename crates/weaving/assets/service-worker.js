console.log('Weaver Service Worker starting');

self.addEventListener('install', (event) => {
	console.log('Weaver Service Worker installing...');
	self.skipWaiting();
});

self.addEventListener('activate', (event) => {
	console.log('Weaver Service Worker activating...');
	event.waitUntil(self.clients.claim());
});

let ws;
function connectWebSocket() {
	ws = new WebSocket('ws://{SERVE_ADDRESS}/ws');

	ws.onopen = () => {
		console.log('Service Worker WebSocket opened!');
	};

	ws.onmessage = (event) => {
		console.log('Message from server (Service Worker WS):', event.data);
		if (event.data === 'reload') {
			self.clients.matchAll().then(clients => {
				clients.forEach(client => {
					client.postMessage('reload');
				});
			});
		}
	};

	ws.onclose = () => {
		console.log('Service Worker WebSocket closed. Reconnecting...');
		setTimeout(connectWebSocket, 1000);
	};

	ws.onerror = (error) => {
		console.error('Service Worker WebSocket error:', error);
	};
}

connectWebSocket();

self.addEventListener('fetch', (event) => {
	{
		event.respondWith(
			caches.match(event.request).then(response => {
				return response || fetch(event.request);
			}).catch(() => {
				return fetch(event.request);
			})
		);
	}
});

