// clientSimulator.js
const io = require('socket.io-client');

const SERVER_URL = 'http://localhost:3000'; // Change this to your server URL
const NUM_CLIENTS = 100000; // Change this to the number of clients you want to simulate

// Function to simulate a client connection
function simulateClientConnection(clientId) {
    const socket = io.connect(SERVER_URL);

    socket.on('connect', () => {
        console.log(`Client ${clientId} connected: ${socket.id}`);

        // Simulate sending data to the server
        setInterval(() => {
            const data = {
                clientId: clientId,
                message: `Hello from Client ${clientId}`
            };
            socket.emit('data', data);
        }, 200); // Sending data every 2 seconds

        // Simulate disconnection after some time

    });
}

// Simulate connections for each client
for (let i = 1; i <= NUM_CLIENTS; i++) {
    simulateClientConnection(i);
}
