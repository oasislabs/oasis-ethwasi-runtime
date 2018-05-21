pragma solidity 0.4.18;
contract Deployed {
    uint public a = 1;
    
    function setA(uint _a) public returns (uint) {
        a = _a;
        return a;
    }
}

contract Existing  {
    Deployed dc;
    
    function Existing(address _t) public {
        dc = Deployed(_t);
    }
 
    function getA() public view returns (uint result) {
        return dc.a();
    }
    
    function setA(uint _val) public returns (uint result) {
        dc.setA(_val);
        return _val;
    }
    
}
