## communication flow

1. http api/ota
   1. http api/ota/activate
1. ws chobits/v1
   1. client hello
   2. server hello

## chat flow

> > -> present client sent message to server
> > <- present server sent message to client

1. listen manual mode

   1. -> Listen(state:start,mode:manual)
   1. -> Voice
   1. -> Listen(state:stop)

1. listen auto mode

   1. -> Voice
   1. -> Listen(state:detect,text:"hello")
   1. -> Listen(state:start,mode:auto)
   1. -> Voice
