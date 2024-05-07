// clientSimulator.js
const io = require('socket.io-client');

const SERVER_URL = 'http://localhost:3000'; // Change this to your server URL
const NUM_CLIENTS = 1000; // Change this to the number of clients you want to simulate

// Function to simulate a client connection
function simulateClientConnection(clientId) {
    const socket = io.connect(SERVER_URL);

    socket.on('connect', () => {
        console.log(`Client ${clientId} connected: ${socket.id}`);

        // Simulate sending data to the server
        setInterval(() => {
            const data = {
                clientId: clientId,
                message: `Hello from Client Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Suscipit tellus mauris a diam maecenas sed enim. Leo integer malesuada nunc vel. Pellentesque habitant morbi tristique senectus et netus et malesuada. At erat pellentesque adipiscing commodo elit. Placerat vestibulum lectus mauris ultrices eros. Nunc non blandit massa enim nec. Nisi lacus sed viverra tellus in hac. Quis ipsum suspendisse ultrices gravida dictum. Tempus iaculis urna id volutpat lacus laoreet non curabitur. Ultricies mi quis hendrerit dolor magna eget est lorem ipsum. Nulla facilisi morbi tempus iaculis urna id. Libero id faucibus nisl tincidunt. Euismod in pellentesque massa placerat duis ultricies. Pretium vulputate sapien nec sagittis aliquam malesuada. Posuere ac ut consequat semper viverra nam. Id nibh tortor id aliquet lectus. Eget nunc scelerisque viverra mauris in.

                Diam quam nulla porttitor massa id neque aliquam vestibulum morbi. Congue nisi vitae suscipit tellus mauris a diam maecenas sed. Feugiat nisl pretium fusce id velit ut. Sapien eget mi proin sed libero enim sed. Faucibus vitae aliquet nec ullamcorper sit. At urna condimentum mattis pellentesque id nibh. Suscipit tellus mauris a diam maecenas sed enim ut. Nunc id cursus metus aliquam eleifend. Netus et malesuada fames ac turpis egestas integer. Orci eu lobortis elementum nibh tellus molestie. Ut eu sem integer vitae justo eget magna fermentum. Sed ullamcorper morbi tincidunt ornare massa eget egestas purus. At elementum eu facilisis sed odio morbi quis commodo odio. Turpis nunc eget lorem dolor sed viverra ipsum.
                
                Ut placerat orci nulla pellentesque dignissim enim sit amet. Nam at lectus urna duis convallis convallis. Pharetra magna ac placerat vestibulum lectus mauris. Nibh mauris cursus mattis molestie. Vitae proin sagittis nisl rhoncus mattis rhoncus urna. Vitae et leo duis ut diam quam nulla porttitor massa. Tortor aliquam nulla facilisi cras fermentum. Volutpat commodo sed egestas egestas fringilla phasellus faucibus. Lacus laoreet non curabitur gravida arcu ac. Ut sem viverra aliquet eget sit amet. Luctus accumsan tortor posuere ac ut consequat semper viverra nam. Nullam ac tortor vitae purus faucibus ornare. Amet volutpat consequat mauris nunc congue nisi vitae suscipit. Varius vel pharetra vel turpis. Sagittis aliquam malesuada bibendum arcu vitae elementum curabitur vitae. Suspendisse sed nisi lacus sed viverra tellus in hac. Elementum pulvinar etiam non quam lacus suspendisse faucibus interdum. Donec et odio pellentesque diam volutpat commodo sed egestas. Sapien et ligula ullamcorper malesuada proin libero nunc.
                
                Sit amet mattis vulputate enim nulla. Cras tincidunt lobortis feugiat vivamus at augue eget arcu. Proin sagittis nisl rhoncus mattis rhoncus urna neque viverra justo. In metus vulputate eu scelerisque felis. Tempor orci eu lobortis elementum nibh. Mi bibendum neque egestas congue quisque. Pharetra magna ac placerat vestibulum lectus mauris ultrices eros. Tincidunt ornare massa eget egestas purus viverra accumsan. Et ligula ullamcorper malesuada proin libero nunc consequat interdum. Ullamcorper dignissim cras tincidunt lobortis feugiat. Bibendum ut tristique et egestas quis ipsum suspendisse. Lorem donec massa sapien faucibus. Morbi non arcu risus quis varius. Eros in cursus turpis massa.
                
                Donec et odio pellentesque diam volutpat commodo sed egestas egestas. Ac turpis egestas sed tempus urna et pharetra. A pellentesque sit amet porttitor. Nisi est sit amet facilisis magna etiam tempor orci. Sodales neque sodales ut etiam sit amet. Sit amet tellus cras adipiscing enim eu. Viverra orci sagittis eu volutpat. Praesent semper feugiat nibh sed pulvinar proin. Integer vitae justo eget magna. Fusce ut placerat orci nulla pellentesque dignissim enim sit amet. In hac habitasse platea dictumst quisque sagittis purus. Mauris cursus mattis molestie a. In hac habitasse platea dictumst vestibulum rhoncus est pellentesque elit. Laoreet suspendisse interdum consectetur libero. Vel pretium lectus quam id. Lacus sed turpis tincidunt id aliquet risus.`
            };
            socket.emit('data', data);
        }, 25); // Sending data every 2 seconds

        // Simulate disconnection after some time

    });
}

// Simulate connections for each client
for (let i = 1; i <= NUM_CLIENTS; i++) {
    simulateClientConnection(i);
}
