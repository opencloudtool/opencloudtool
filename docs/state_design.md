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
Create a new package for communication with state files

Call code from state package from oct-cloud

Also we need config file for set related resource for each resource

For each structure will be a struct and in this struct will be model like Django DB model. 
In this model we need descibe state structure. It means which fields we should save after create resource
For interaction with state file we can get inspired get inspired by the ORM approach to databases

Example:

For EC2 instance we need know him instance profile
For Instance profile we need know IAM roles

```
class State(ABC):
    enum ResourceType {
        EC2,
        IAMRole,
        InstanceProfile,
    }
        
    struct Model {}

	fn add():
		...
		
	fn update():
		...
		
	fn remove():
		...

struct EC2State(State):
    struct Model {
        let mut id: String
        let mut arn: String
        let mut public_ip: String
        let mut public_dns: String
        let mut related_resources: [HashMap<String, ResourceType>] = [{"instance_profile": ResourceType::IAMRole}]  
    }
    
	fn add():
        // save to state file with self.Model fields
		
	fn update():
		// update exists state file
		
	fn remove():
		// remove resource from state file


impl InstanceProfileState {
    let mut related_resources: [ResourceType] = [ResourceType::IAM]
    struct Model {
        let mut name: String
        let mut related_resources: [HashMap<String, ResourceType>] = [{"roles": ResourceType::IAMRole}]  
    }
    
    fn add():
        // save to state file with self.Model fields
    
    fn update():
        // update exists state file
    
    fn remove():
        // remove resource from state file
}
```

When we create EC2 instance we need create IAM role for it

User story:

user -> oct-cli -> oct-cloud -> oct-state -> oct-cloud -> oct-state

User call oct-cli with `deploy` command
Oct-cli call oct-cloud `create` function
Oct-cloud get state struct for this resource
Oct-cloud see which resources related to this resource and create it
Oct-cloud call oct-state `add` function for each resource
Oct-state save data to state file

How it will look like:

```
# utils.rs

fn get_resource_struct(resource_type: ResourceType) -> Struct {
    match resource_type {
        ResourceType::EC2 => EC2InstanceImpl,
        ResourceType::IAMRole => IAMRoleImpl,
        ResourceType::InstanceProfile => InstanceProfileImpl,
    }
}


impl Resource for EC2Instance {
    let mut created_resources: HashMap<ResourceType, []> = HashMap::new()
    let state_storage: EC2State = EC2State::new()

    fn create():
        for related_resource in self.state_storage.Model.related_resources:
            let resource_struct = get_resource_struct(related_resource)
            response = resource_struct.create()
            self.created_resources.push(related_resource)
        
        self.client.run_instances(instance_profile=self.created_resources[ResourceType::InstanceProfile].name)
        
        state.add(created_resources)
}
```

## Alternatives considered

- Terraform and analogs
- Don't store state
