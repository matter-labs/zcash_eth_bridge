// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script, console2} from "forge-std-1.12.0/Script.sol";

import {ZcashBridge} from "src/ZcashBridge.sol";
import {WZec} from "src/WZec.sol";

contract DeployBridge is Script {
    function run() external {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");

        vm.startBroadcast(deployerKey);

        WZec token = new WZec();
        ZcashBridge bridge = new ZcashBridge(address(token));
        token.setBridge(address(bridge));

        console2.log("WZec:", address(token));
        console2.log("ZcashBridge:", address(bridge));

        vm.stopBroadcast();
    }
}
