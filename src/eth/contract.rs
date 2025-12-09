use alloy::sol;

sol!(
    #[sol(rpc)]
    ZcashBridge,
    "./contracts/out/ZcashBridge.sol/ZcashBridge.json"
);

sol!(
    #[sol(rpc)]
    WZec,
    "./contracts/out/WZec.sol/WZec.json"
);
