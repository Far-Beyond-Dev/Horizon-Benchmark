const io = require('socket.io-client');

const SERVER_URL = 'http://localhost:3000'; // Change this to your server URL
const NUM_CLIENTS = 1; // Change this to the number of clients you want to simulate
const SEND_INTERVAL = 2000; // Interval in milliseconds for sending data

// Function to generate custom message and body
function generateCustomMessage(clientId) {
    return {
        eventName: 'whisper', // You can change this to any event name you want
        body: {
            clientId: clientId,
            timestamp: Date.now(),
            message: `Custom message from Client ${clientId}`,
            data: {
                value: Math.random(),
                isActive: true,
                tags: ['test', 'custom', `client${clientId}`]
            }
        }
    };
}

// Function to simulate a client connection
function simulateClientConnection(clientId) {
    let attemptCount = 0;
    const socket = io.connect(SERVER_URL, {
        reconnection: true,
        reconnectionAttempts: 5,
        reconnectionDelay: 1000
    });

    const logConnectingMessage = () => {
        attemptCount++;
        console.log(`Client ${clientId} connecting... Attempt ${attemptCount}`);
    };

    socket.on('connect', () => {
        attemptCount = 0;
        console.log(`Client ${clientId} connected: ${socket.id}`);

        // Simulate sending custom data to the server
        const intervalId = setInterval(() => {
            const customMessage = generateCustomMessage(clientId);
            socket.emit(customMessage.eventName, customMessage.body);
            console.log(`Client ${clientId} sent: ${JSON.stringify(customMessage)}`);
        }, SEND_INTERVAL);

        socket.on('disconnect', (reason) => {
            console.log(`Client ${clientId} disconnected: ${reason}`);
            clearInterval(intervalId);
        });

        socket.on('error', (error) => {
            console.error(`Client ${clientId} error: ${error}`);
        });
    });

    logConnectingMessage();
    socket.on('reconnect_attempt', logConnectingMessage);

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