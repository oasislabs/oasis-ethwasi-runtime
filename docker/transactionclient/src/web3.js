/**
 * This script runs as a directly executed binary to drive transactions on a
 * Selected ethereum network. It operates the truffle HDWallet provider to
 * send 0-value transactions to itself at a fixed interval.
 */

var pause = async function (timeout) {
    return new Promise(function (resolve, _reject) {
        setTimeout(resolve, timeout);
    });
};

var makeTxn = function (client) {
    return new Promise(function (resolve, reject) {
        client.eth.sendTransaction({
            from: "1cca28600d7491365520b31b466f88647b9839ec",
            gas: 100000,
            gasPrice: 1000000000,
            to: "1cca28600d7491365520b31b466f88647b9839ec"
        }, function (err, txn) {
            if (err) {
                reject(err);
            } else {
                resolve(txn);
            }
        });
    });
};

// Only run if directly executed.
if (require.main === module) {
    let HDWalletProvider = require("truffle-hdwallet-provider");
    let web3 = require('web3');
    const client = require('prom-client');
    const txncount = new client.Counter({
        name: 'txncount',
        help: 'Number of web3 transactions made'
    });
    const txnerrors = new client.Counter({
        name: 'txnerrors',
        help: 'Number of errored web3 transactions'
    });
    const txnlatency = new client.Summary({
        name: 'txnlatencysummary',
        help: 'Latency summary of web3 transactions',
    });

    let mnemonic = 'patient oppose cotton portion chair gentle jelly dice supply salmon blast priority';

    if (process.argv.length < 4) {
        console.warn("Usage: web3.js <provider> <interval-ms> [pushgateway address] [push job prefix] [push instance label]");
        process.exit(0);
    }
    let gateway = null;
    let pushJobPrefix = "";
    let pushGroupings = {};
    if (process.argv.length < 5) {
        const http = require('http');
        let server = http.createServer((req, res) => {
            res.end(client.register.metrics());
        });
        server.listen(3000);
    } else if (process.argv.length == 7) {
        gateway = new client.Pushgateway(process.argv[4]);
        pushJobPrefix = process.argv[5];
        pushGroupings['instance'] = process.argv[6];
    } else {
        console.warn("Usage: web3.js <provider> <interval-ms> [pushgateway address] [push job prefix] [push instance label]");
        process.exit(0);
    }

    let run = async function () {
        let provider = new HDWalletProvider(mnemonic, process.argv[2]);
        let client = new web3();
        await client.setProvider(provider);
        while (true) {
            await pause(process.argv[3]);
            const end = txnlatency.startTimer();
            try {
                await makeTxn(client);
            } catch (e) {
                console.warn("Transaction failed.", e);
                txnerrors.inc();
            }
            end();
            txncount.inc();
            if (gateway != null) {
                gateway.pushAdd({ jobName: `${pushJobPrefix}-web3-txn`, groupings: pushGroupings }, () => { });
            }
        }
    };

    run();
}
