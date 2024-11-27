In the state we are storage resources data.
For each resource we need save related resources

### Ideas
Add resource change history

For each resource we should know fields, for example for ec2 we are need ecr repository. Each resource will be know self dependencies  

## Objective (goal)
We are need storage 

We need to store the current state of the raised resources so that we can manage them effectively.
So that when deleting/stopping resources, the resources associated with it are also deleted/stopped

## Background (why you decided doing that)
Now, to delete resources, we need to edit the current code and manually insert the resource IDs we want to delete. When there are a lot of resources, this approach will be very slow.
There is a chance to make a mistake and delete the wrong resource.
Associated resources also have to be deleted manually

## Requirements
- [ ] The infrastructure state is stored in a file

## Design (you implementation proposal, use diagrams, maybe code snippets)
Here will be solution description

Create a new package for communication with state files

For the first version we can create state file after create resource

After then we can move this code to function and call it from oct-cloud

Кем это будет вызываться?
add_resource
remove_resource
```
class State(ABC):
	fn create():
		...
		
	fn update():
		...
		
	fn destroy():
		...

class EC2State(State):
	fn create():
		// impl of super create function
		
	fn update():
		// impl of super update function
		
	fn destroy():
		// impl of super destroy function
```

## Alternatives considered

- Terraform and analogs
