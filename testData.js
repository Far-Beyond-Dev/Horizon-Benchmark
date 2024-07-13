const sqlite3 = require('sqlite3').verbose();
const { SingleBar } = require('cli-progress');

// Helper function to generate UUIDs
function generateUUID() {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
        var r = Math.random() * 16 | 0, v = c == 'x' ? r : (r & 0x3 | 0x8);
        return v.toString(16);
    });
}

// Helper function to generate random names
function generateRandomName() {
    const adjectives = ['Red', 'Blue', 'Green', 'Yellow', 'Orange', 'Purple', 'Silver', 'Golden', 'Crystal'];
    const nouns = ['Planet', 'Star', 'Moon', 'Galaxy', 'Nebula', 'Comet', 'Asteroid', 'Satellite'];
    const adj = adjectives[Math.floor(Math.random() * adjectives.length)];
    const noun = nouns[Math.floor(Math.random() * nouns.length)];
    return `${adj} ${noun}`;
}

// Helper function to generate random voxel data overrides
function generateRandomVoxelData() {
    const types = ['rock', 'sand', 'water', 'ice', 'grass', 'metal'];
    const randomType = types[Math.floor(Math.random() * types.length)];
    const hardness = Math.floor(Math.random() * 10) + 1; // Random hardness from 1 to 10
    return { type: randomType, hardness };
}

// Main save system
class SaveSystem {
    constructor(dbPath) {
        this.db = new sqlite3.Database(dbPath);
        this.initializeTables();
    }

    const fs = require('fs');
    const sqlite3 = require('sqlite3').verbose();
    const { SingleBar } = require('cli-progress');
    
    // Connect to SQLite database
    const db = new sqlite3.Database('game_data.db');
    
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
    
    // Function to execute a query with progress bar
    function executeQuery(query, progressBar, onDataReceived, onComplete) {
        db.all(query, [], (err, rows) => {
            if (err) {
                console.error('Error querying database:', err.message);
                return;
            }
    
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
    const planetsData = {};
    executeQuery(planetsQuery, progressBarPlanets, row => {
        const { planet_uuid, planet_name, voxel_data } = row;
    
        planetsData[planet_uuid] = {
            uuid: planet_uuid,
            name: planet_name,
            voxelDataOverrides: JSON.parse(voxel_data)
        };
    }, checkQueryCompletion);
    
    // Execute players query
    const playersData = {};
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
        // Prepare text content for file
        let content = '=== Game Data Visualization ===\n\n';
    
        // Add planets and their voxel data overrides
        content += '--- Planets and Voxel Data Overrides ---\n\n';
        let planetHeaders = ['Planet UUID', 'Planet Name', 'Voxel X', 'Voxel Y', 'Voxel Z', 'Voxel Data'];
        let planetRows = [];
        Object.values(planetsData).forEach(planet => {
            planet.voxelDataOverrides.forEach(override => {
                planetRows.push([
                    planet.uuid,
                    planet.name,
                    override.x,
                    override.y,
                    override.z,
                    JSON.stringify(override.data)
                ]);
            });
        });
        content += generateTable(planetHeaders, planetRows);
    
        // Add players and their inventory
        content += '--- Players and Inventory ---\n\n';
        let playerHeaders = ['Player UUID', 'Player Name', 'Item Name', 'Item Quantity'];
        let playerRows = [];
        Object.values(playersData).forEach(player => {
            player.inventory.forEach(item => {
                playerRows.push([
                    player.uuid,
                    player.name,
                    item.name,
                    item.quantity
                ]);
            });
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
    

        this.db.serialize(() => {
            this.db.run(createPlanetsTableQuery);
            this.db.run(createPlayersTableQuery);
            this.db.run(createInventoryTableQuery);
        });
    }

    addPlanet(name, voxelData, callback) {
        const uuid = generateUUID();
        const insertPlanetQuery = 'INSERT INTO planets (uuid, name, voxel_data) VALUES (?, ?, ?)';
        this.db.run(insertPlanetQuery, [uuid, name, JSON.stringify(voxelData)], callback);
        return uuid;
    }

    addPlayer(name, callback) {
        const uuid = generateUUID();
        const insertPlayerQuery = 'INSERT INTO players (uuid, name) VALUES (?, ?)';
        this.db.run(insertPlayerQuery, [uuid, name], (err) => callback(err, uuid));
    }

    addItemToPlayerInventoryBatch(playerUUID, items, callback) {
        const insertItemQuery = 'INSERT INTO inventory (player_uuid, item_name, quantity) VALUES (?, ?, ?)';
        const stmt = this.db.prepare(insertItemQuery);
        items.forEach(({ name, quantity }, index) => {
            stmt.run(playerUUID, name, quantity, (err) => {
                if (index === items.length - 1) {
                    stmt.finalize(callback);
                }
            });
        });
    }

    closeDatabase(callback) {
        this.db.close(callback);
    }

    // Generate random save data
    generateRandomSaveData(numPlanets, numPlayers, progressBar, callback) {
        this.db.serialize(() => {
            this.db.run('BEGIN TRANSACTION', (err) => {
                if (err) return callback(err);

                let completed = 0;

                // Generate random planets
                for (let i = 0; i < numPlanets; i++) {
                    const planetName = generateRandomName();
                    const voxelOverrides = [];
                    const numOverrides = Math.floor(Math.random() * 100000) + 1;
                    for (let j = 0; j < numOverrides; j++) {
                        const x = Math.floor(Math.random() * 10000);
                        const y = Math.floor(Math.random() * 10000);
                        const z = Math.floor(Math.random() * 10000);
                        const voxelData = generateRandomVoxelData();
                        voxelOverrides.push({ x, y, z, data: voxelData });
                    }
                    this.addPlanet(planetName, voxelOverrides, (err) => {
                        if (err) return callback(err);
                        completed++;
                        progressBar.increment();
                        if (completed === numPlanets) {
                            progressBar.stop();

                            // Generate random players
                            progressBar.start(numPlayers, 0);
                            let playerCompleted = 0;
                            for (let i = 0; i < numPlayers; i++) {
                                const playerName = `Player${i + 1}`;
                                this.addPlayer(playerName, (err, playerUUID) => {
                                    if (err) return callback(err);

                                    const items = [];
                                    const numItems = Math.floor(Math.random() * 15) + 1;
                                    for (let j = 0; j < numItems; j++) {
                                        const itemName = `Item${j + 1}`;
                                        const quantity = Math.floor(Math.random() * 500) + 1;
                                        items.push({ name: itemName, quantity });
                                    }

                                    this.addItemToPlayerInventoryBatch(playerUUID, items, (err) => {
                                        if (err) return callback(err);
                                        playerCompleted++;
                                        progressBar.increment();
                                        if (playerCompleted === numPlayers) {
                                            progressBar.stop();
                                            this.db.run('COMMIT', callback);
                                        }
                                    });
                                });
                            }
                        }
                    });
                }
            });
        });
    }
}

// Example usage: generate random save data and store in SQLite
const saveSystem = new SaveSystem('game_data.db');

// Initialize progress bar for planets and players
const totalPlanets = 30;
const totalPlayers = 5000;
const progressBar = new SingleBar({
    format: 'Generating |{bar}| {percentage}% | ETA: {eta}s | {value}/{total}',
    barCompleteChar: '\u2588',
    barIncompleteChar: '\u2591',
    hideCursor: true
});
progressBar.start(totalPlanets + totalPlayers, 0);

saveSystem.generateRandomSaveData(totalPlanets, totalPlayers, progressBar, (err) => {
    if (err) {
        console.error('Error generating random save data:', err);
    }
    // Close the database connection when done
    saveSystem.closeDatabase((err) => {
        if (err) {
            console.error('Error closing database:', err);
        }
    });
});
