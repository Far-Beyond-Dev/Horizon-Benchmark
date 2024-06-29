const io = require('socket.io-client');

const SERVER_URL = 'http://localhost:3000'; // Change this to your server URL
const NUM_CLIENTS = 10; // Change this to the number of clients you want to simulate
const SEND_INTERVAL = 2000; // Interval in milliseconds for sending data

// Function to simulate a client connection
function simulateClientConnection(clientId) {
    let attemptCount = 0;
    const socket = io.connect(SERVER_URL, {
        reconnection: true, // Ensure reconnection attempts
        reconnectionAttempts: 5, // Number of reconnection attempts
        reconnectionDelay: 1000 // Delay between reconnection attempts
    });

    const logConnectingMessage = () => {
        attemptCount++;
        console.log(`Client ${clientId} connecting... Attempt ${attemptCount}`);
    };

    socket.on('connect', () => {
        attemptCount = 0; // Reset attempt count on successful connection
        console.log(`Client ${clientId} connected: ${socket.id}`);

        // Simulate sending data to the server
        const intervalId = setInterval(() => {
            const data = {
                clientId: clientId,
                message: `Hello from Client ${clientId}`
            };
            socket.emit('data', data);
        }, SEND_INTERVAL); // Sending data every 2 seconds

        // Handle disconnection
        socket.on('disconnect', (reason) => {
            console.log(`Client ${clientId} disconnected: ${reason}`);
            clearInterval(intervalId);
        });

        // Handle errors
        socket.on('error', (error) => {
            console.error(`Client ${clientId} error: ${error}`);
        });
    });

    // Handle initial connection attempts
    logConnectingMessage();

    // Handle reconnection attempts
    socket.on('reconnect_attempt', logConnectingMessage);

    // Clean up on process termination
    process.on('SIGINT', () => {
        console.log(`Client ${clientId} terminating...`);
        socket.disconnect();
        process.exit();
    });
}

// Simulate connections for each client
for (let i = 1; i <= NUM_CLIENTS; i++) {
    simulateClientConnection(i);
}
