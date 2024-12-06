### Ideas
Deploy user apps in cloud machine through oct agent

## Objective (goal)
Runs ans manage user apps in deployment machines

## Background (why you decided doing that)
Without function our tool will be useless

## Requirements
- [ ] Config file with description user apps
- [ ] Successful deployment
- [ ] Available endpoints in oct-ctl for run ans stop containers

## Design (you implementation proposal, use diagrams, maybe code snippets)
- [ ] User should create config file and describe available apps
- [ ] Oct-ctl should have endpoints for run and stop containers
- [ ] Oct-cli call oct-cloud for create machine in the cloud 
- [ ] After creating machine oct-cloud reads config file and gets apps
- [ ] Oct-cloud sends http request to oct-ctl for creates containers for apps

How get user apps info to oct-ctl?
We have custom user file and instructions for run him app 
When we deployment oct-ctl we should keep this app to machine
After deployment oct-ctl user app will be able in machine
When we call endpoint agent run app with instructions

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
