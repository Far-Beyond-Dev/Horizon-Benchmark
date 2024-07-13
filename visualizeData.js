const fs = require('fs');
const sqlite3 = require('sqlite3').verbose();
const { SingleBar } = require('cli-progress');

// Connect to SQLite database
const db = new sqlite3.Database('game_data.db');

db.all("SELECT COUNT(*) as count FROM players", [], (err, rows) => {
    if (err) {
        console.error("Error counting players:", err);
    } else {
        console.log("Number of players in database:", rows[0].count);
    }
});

// Query to retrieve planets and their voxel data overrides
const planetsQuery = `
    SELECT
        p.uuid AS planet_uuid, p.name AS planet_name, p.voxel_data AS voxel_data
    FROM planets p
`;

// Query to retrieve players and their inventories
const playersQuery = `
    SELECT
        pl.uuid AS player_uuid, pl.name AS player_name,
        i.item_name AS item_name, i.quantity AS item_quantity
    FROM players pl
    LEFT JOIN inventory i ON pl.uuid = i.player_uuid
`;

// Initialize progress bars
const progressBarPlanets = new SingleBar({
    format: 'Planets |{bar}| {percentage}% | {value}/{total} Queries',
    barCompleteChar: '\u2588',
    barIncompleteChar: '\u2591',
    hideCursor: true
});

const progressBarPlayers = new SingleBar({
    format: 'Players |{bar}| {percentage}% | {value}/{total} Queries',
    barCompleteChar: '\u2588',
    barIncompleteChar: '\u2591',
    hideCursor: true
});

// Maximum size allowed for voxel data (adjust as needed)
const MAX_VOXEL_DATA_SIZE = 100; // Example maximum size

// Data storage for planets and players
const planetsData = {};
const playersData = {};

// Function to execute a query with progress bar
function executeQuery(query, progressBar, onDataReceived, onComplete) {
    console.log("Executing query:", query);
    db.all(query, [], (err, rows) => {
        if (err) {
            console.error('Error querying database:', err.message);
            return;
        }
        console.log(`Query returned ${rows.length} rows`);

        progressBar.start(rows.length, 0);

        rows.forEach((row, index) => {
            onDataReceived(row);
            progressBar.update(index + 1);
        });

        progressBar.stop();
        onComplete();
    });
}

// Execute planets query
executeQuery(planetsQuery, progressBarPlanets, row => {
    const { planet_uuid, planet_name, voxel_data } = row;

    // Check if voxel data exceeds maximum size
    if (voxel_data && voxel_data.length > MAX_VOXEL_DATA_SIZE) {
        planetsData[planet_uuid] = {
            uuid: planet_uuid,
            name: planet_name,
            voxelData: "OVERFLOW" // Placeholder for overflow case
        };
    } else {
        planetsData[planet_uuid] = {
            uuid: planet_uuid,
            name: planet_name,
            voxelData: voxel_data ? JSON.parse(voxel_data) : []
        };
    }
}, checkQueryCompletion);

// Execute players query
executeQuery(playersQuery, progressBarPlayers, row => {
    const { player_uuid, player_name, item_name, item_quantity } = row;

    if (!playersData[player_uuid]) {
        playersData[player_uuid] = {
            uuid: player_uuid,
            name: player_name,
            inventory: []
        };
    }

    if (item_name !== null && item_quantity !== null) {
        playersData[player_uuid].inventory.push({
            name: item_name,
            quantity: item_quantity
        });
    }
}, checkQueryCompletion);

// Function to generate ASCII table
function generateTable(headers, rows) {
    const colWidths = headers.map((header, i) => Math.max(header.length, ...rows.map(row => row[i].toString().length)));
    const separator = '+' + colWidths.map(w => '-'.repeat(w + 2)).join('+') + '+\n';
    const headerRow = '| ' + headers.map((header, i) => header.padEnd(colWidths[i])).join(' | ') + ' |\n';
    const dataRows = rows.map(row => '| ' + row.map((cell, i) => cell.toString().padEnd(colWidths[i])).join(' | ') + ' |\n').join('');
    return separator + headerRow + separator + dataRows + separator;
}

// Handle completion of all queries
function handleQueryCompletion() {
    console.log("Handling query completion");
    console.log("Players data at completion:", JSON.stringify(playersData, null, 2));

    if (Object.keys(playersData).length === 0) {
        console.error("No player data available. Aborting visualization.");
        return;
    }
    // Prepare text content for file
    let content = '=== Game Data Visualization ===\n\n';

    // Add planets and their voxel data overrides
    content += '--- Planets and Voxel Data ---\n\n';
    let planetHeaders = ['Planet UUID', 'Planet Name', 'Voxel Data'];
    let planetRows = [];
    Object.values(planetsData).forEach(planet => {
        if (planet.voxelData === "OVERFLOW") {
            planetRows.push([
                planet.uuid,
                planet.name,
                "OVERFLOW"
            ]);
        } else {
            planet.voxelData.forEach(data => {
                planetRows.push([
                    planet.uuid,
                    planet.name,
                    JSON.stringify(data)
                ]);
            });
        }
    });
    content += generateTable(planetHeaders, planetRows);

    // Add players and their inventory
    console.log("Players data before table generation:", JSON.stringify(playersData, null, 2));
    content += '--- Players and Inventory ---\n\n';
    let playerHeaders = ['Player UUID', 'Player Name', 'Item Name', 'Item Quantity'];
    let playerRows = [];
    Object.values(playersData).forEach(player => {
        if (player.inventory.length === 0) {
            playerRows.push([
                player.uuid,
                player.name,
                'No items',
                '0'
            ]);
        } else {
            player.inventory.forEach(item => {
                playerRows.push([
                    player.uuid,
                    player.name,
                    item.name,
                    item.quantity
                ]);
            });
        }
    });
    content += generateTable(playerHeaders, playerRows);

    // Write content to file
    const fileName = 'visualization.txt';
    fs.writeFile(fileName, content, err => {
        if (err) {
            console.error('Error writing to file:', err);
            return;
        }
        console.log(`Visualization data written to ${fileName}`);
        
        // Close the database connection
        db.close();
    });
}

// Ensure both progress bars complete before finalizing
let queriesCompleted = 0;
function checkQueryCompletion() {
    queriesCompleted++;
    if (queriesCompleted === 2) {
        handleQueryCompletion();
    }
}

progressBarPlanets.on('stop', checkQueryCompletion);
progressBarPlayers.on('stop', checkQueryCompletion);
