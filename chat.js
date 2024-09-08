const io = require('socket.io-client');
const readline = require('readline');

const socket = io('http://localhost:3000'); // Replace with your server URL

const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout
});

console.log('Chat CLI Tool');
console.log('Commands:');
console.log('/whisper <recipient> <message> - Send a private message');
console.log('/broadcast <message> - Send a message to all users');
console.log('/help - Request help');
console.log('/quit - Exit the application');

socket.on('connect', () => {
  console.log('Connected to the server');
  promptUser();
});

socket.on('whisper', (data) => {
  console.log(`Whisper from ${data.sender}: ${data.message}`);
});

socket.on('broadcast', (message) => {
  console.log(`Broadcast: ${message}`);
});

socket.on('help', (message) => {
  console.log(`Help: ${message}`);
});

function promptUser() {
  rl.question('> ', (input) => {
    const [command, ...args] = input.split(' ');

    switch (command) {
      case '/whisper':
        if (args.length < 2) {
          console.log('Usage: /whisper <recipient> <message>');
        } else {
          const recipient = args[0];
          const message = args.slice(1).join(' ');
          socket.emit('whisper', { recipient, message });
        }
        break;
      case '/broadcast':
        if (args.length < 1) {
          console.log('Usage: /broadcast <message>');
        } else {
          const message = args.join(' ');
          socket.emit('broadcast', message);
        }
        break;
      case '/help':
        socket.emit('help');
        break;
      case '/quit':
        console.log('Disconnecting...');
        socket.disconnect();
        rl.close();
        return;
      default:
        console.log('Unknown command. Type /help for available commands.');
    }

    promptUser();
  });
}

socket.on('disconnect', () => {
  console.log('Disconnected from the server');
});