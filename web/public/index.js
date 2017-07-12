'use strict'

let io = null

let reconnectingText = null
const MIN_WAIT = 100
let wait = MIN_WAIT

let states = []

function resetWait() {
    wait = MIN_WAIT
}

function incrementWait() {
    const MAX_WAIT = 2000
    if (wait < MAX_WAIT)
        wait *= 1.25
    else
        wait = MAX_WAIT
}

function connect(address) {
    io = new WebSocket(address)
    io.onopen = () => {
        resetWait()
        if (reconnectingText) {
            app.stage.removeChild(reconnectingText)
            reconnectingText = null
        }
    };

    io.onmessage = (msg) => {
        states.push(JSON.parse(msg.data))
    }

    io.onclose = () => {
        if (!reconnectingText && app) {
            reconnectingText = new PIXI.Text('Reconnecting...')
            reconnectingText.anchor.set(0.5)
            reconnectingText.y = 500
            reconnectingText.x = 400
            app.stage.addChild(reconnectingText)
        }
        io = null
        setTimeout(() => {
            incrementWait()
            connect(address);
        }, wait)
    };

    io.onerror = (err) => {
        console.error('Socket encountered error: ', err.message, 'Closing socket')
        io.close()
        io = null
    };
}



function send(data) {
    if (io && io.readyState === 1) {
        console.log('sending data: ', data)
        io.send(data)
    }
}

const Application = PIXI.Application,
    loader = PIXI.loader,
    resources = PIXI.loader.resources,
    Sprite = PIXI.Sprite

const app = new Application(
    1000, 1000, {
        backgroundColor: 0x1099bb,
        autoResize: true,
        antialias: true,
        resolution: 1,
    },
)

const addr = window.location.hostname === 'localhost' ? 'ws://localhost:8081' : 'wss://beta.jnferner.com/socket'
connect(addr)
document.body.appendChild(app.view)

loader
    .add([{
        name: 'dungeonAtlas',
        url: 'assets/dungeon.json'
    }, ])
    .on('progress', loadProgressHandler)
    .load(setup)

document.addEventListener("keydown", (event) => {
    send('keydown:' + event.key)
})
document.addEventListener("keyup", (event) => {
    send('keyup:' + event.key)
})

function loadProgressHandler(loader, resource) {
    console.log('loading: ' + resource.name + ' (' + resource.url + ')')
    console.log('progress: ' + loader.progress + '%')
    if (resource.error)
        console.error(resource.error)
}

let blob

function setup() {
    blob = new Sprite(resources.dungeonAtlas.textures['blob.png'])
    blob.anchor.set(0.5)
    app.stage.addChild(blob)
    app.ticker.add(gameLoop)
}

let state = connecting

function gameLoop(delta) {
    state()
}

let connectingText
function connecting() {
    if (!connectingText) {
        connectingText = new PIXI.Text('Connecting...')
        connectingText.anchor.set(0.5)
        connectingText.y = 500
        connectingText.x = 400
        app.stage.addChild(connectingText)
    }

    const renderTime = getRenderTime()
    const index = getIndexOfRenderState(states, renderTime)
    if (index > 0) {
        app.stage.removeChild(connectingText)
        state = play
    }
}

function play() {
    render(states)
}

const INTERPOLATION_DELTA = 100

function render(states) {
    const renderTime = getRenderTime()
    const index = getIndexOfRenderState(states, renderTime)
    if (index <= 0)
        return
    states.splice(0, index)
    let interpolatedState = getInterpolatedState(states[0], states[1], renderTime)
    setWorld(interpolatedState)
}

function getRenderTime() {
    const now = new Date().getTime()
    return now - INTERPOLATION_DELTA
}

function getIndexOfRenderState(states, renderTime) {
    const found = states.findIndex((state) => state.timestamp >= renderTime)
    return found - 1
}

function getInterpolatedState(from, to, renderTime) {
    const total = to.timestamp - from.timestamp
    const progress = renderTime - from.timestamp
    if (total === 0 || progress === 0)
        return from
    const fraction = progress / total
    let state = from
    state.vel.x += (to.vel.x - from.vel.x) * fraction
    state.vel.y += (to.vel.y - from.vel.y) * fraction
    state.pos.x += (to.pos.x - from.pos.x) * fraction
    state.pos.y += (to.pos.y - from.pos.y) * fraction
    state.timestamp = renderTime
    return state
}


function setWorld(state) {
    blob.x = state.pos.x
    blob.y = state.pos.y
}