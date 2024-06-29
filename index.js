// clientSimulator.js
const io = require('socket.io-client');

const SERVER_URL = 'http://localhost:3000'; // Change this to your server URL
const NUM_CLIENTS = 10; // Change this to the number of clients you want to simulate

// Function to simulate a client connection
function simulateClientConnection(clientId) {
    const socket = io.connect(SERVER_URL);

    socket.on('connect', () => {
        console.log(`Client ${clientId} connected: ${socket.id}`);

        // Simulate sending data to the server
        setInterval(() => {
            const data = {
                clientId: clientId,
                message: `Hello from Client Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Suscipit tellus mauris a diam maecenas sed enim. Leo integer malesuada nunc vel. Pellentesque habitant morbi tristique senectus et netus et malesuada. At erat pellentesque adipiscing commodo elit. Placerat vestibulum lectus mauris ultrices eros. Nunc non blandit massa enim nec. Nisi lacus sed viverra tellus in hac. Quis ipsum suspendisse ultrices gravida dictum. Tempus iaculis urna id volutpat lacus laoreet non curabitur. Ultricies mi quis hendrerit dolor magna eget est lorem ipsum. Nulla facilisi morbi tempus iaculis urna id. Libero id faucibus nisl tincidunt. Euismod in pellentesque massa placerat duis ultricies. Pretium vulputate sapien nec sagittis aliquam malesuada. Posuere ac ut consequat semper viverra nam. Id nibh tortor id aliquet lectus. Eget nunc scelerisque viverra mauris in.`
            };
            socket.emit('data', data);
        }, 20); // Sending data every 2 seconds

        // Simulate disconnection after some time

    });
}

// Simulate connections for each client
for (let i = 1; i <= NUM_CLIENTS; i++) {
    simulateClientConnection(i);
}
