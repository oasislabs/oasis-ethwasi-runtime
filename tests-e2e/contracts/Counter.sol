pragma solidity ^0.4.0;

contract Counter {
  event Incremented(uint newCounter);

  uint256 _counter;

  function getCounter() public view returns (uint256) {
	return _counter;
  }

  function incrementCounter() public {
	_counter += 1;
	emit Incremented(_counter);
  }

  function incrementCounterManyTimes(uint256 count) public {
	for (uint256 i = 0; i < count; i++) {
	  _counter += 1;
	  emit Incremented(_counter);
	}
  }
}
