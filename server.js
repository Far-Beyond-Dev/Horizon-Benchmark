const io = require('socket.io')(3000, {
    cors: {
        origin: "*",
    }
});

io.on('connection', (socket) => {
    console.log(`Client connected: ${socket.id}`);

    // Handle incoming data messages
    socket.on('data', (data) => {
        console.log(`Received data from client ${data.clientId}: ${data.message}`);
    });

    // Handle client disconnection
    socket.on('disconnect', (reason) => {
        console.log(`Client ${socket.id} disconnected: ${reason}`);
    });

    // Handle errors
    socket.on('error', (error) => {
        console.error(`Client ${socket.id} error: ${error}`);
    });
});

console.log('Server is running on port 3000');
