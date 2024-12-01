In the state we are storage resources data.
For each resource we need save related resources

### Ideas
Add resource storage actual infrastructure state

For each resource we should know fields, for example for ec2 we are need ecr repository.
Each resource will be know self dependencies  

## Objective (goal)
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

Call code from state package from oct-cloud

Also we need config file for set related resource for each resource

Example:

For EC2 instance we need know him IAM roles

```
class State(ABC):
    enum ResourceType {
        EC2,
        ECR,
        IAM,
        InstanceProfile
    }

    let mut related_resources: [ResourceType] = [] // mandatory

	fn add():
		...
		
	fn update():
		...
		
	fn remove():
		...

class EC2State(State):
    let mut related_resources: [ResourceType] = [ResourceType::InstanceProfile]

	fn add():
        // save to state file
		
	fn update():
		// update exists state file
		
	fn remove():
		// remove resource from state file
```

impl InstanceProfileState {
    let mut related_resources: [ResourceType] = [ResourceType::IAM]
    
    fn add():
        // save to state file
    
    fn update():
        // update exists state file
    
    fn remove():
        // remove resource from state file
    
}
    

When we create EC2 instance we need create IAM role for it

User story:

user -> oct-cli -> oct-cloud -> oct-state -> oct-cloud -> oct-state

User call oct-cli with deploy command
Oct-cli call oct-cloud create function
Oct-cloud get state config for this resource
Oct-cloud see which resources related to this resource and create it
Oct-cloud call oct-state create function
Oct-state save data to state file


How it will look like:

impl Resource for EC2Instance {
    let mut created_resources: HashMap<ResourceType, []> = HashMap::new()
    let state_storage: EC2State = EC2State::new()

    fn create():
        for related_resource in self.state_storage.related_resources:
            response = self.client.run_instances(...)
            self.created_resources.push(related_resource)
        
        state.add(created_resources)
}


## Alternatives considered

- Terraform and analogs
- Don't store state
