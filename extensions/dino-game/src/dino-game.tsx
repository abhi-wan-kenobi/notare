import "./dino.css";

import React, { useEffect, useRef } from "react";

// Import sprite image
import spriteImage from "../assets/dino-game-sprite.png";

const DinoGame: React.FC = () => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const gameStateRef = useRef({
    scoreInterval: 0,
    frameInterval: 0,
    groundscroll: 0,
    groundscroll2: 0,
    tempstart: 0,
    groundbool: false,
    frame: 0,
    bool: false,
    grav: 0.6,
    gamespeed: 0,
    multiS: -1,
    picS: 0,
    multiB: -1,
    picB: 0,
    onG: false,
    sprImg: new Image(),
    isButtonHovered: false,
    isDucking: false,
    isJumping: false,
  });

  const obsS = {
    x: 20,
    y: 230,
    w: 34,
    h: 70,
    scroll: -100,
    on: false,
  };

  const obsB = {
    x: 20,
    y: 201,
    w: 49,
    h: 100,
    scroll: -200,
    on: false,
  };

  const player = {
    x: 100,
    y: 500,
    w: 89,
    h: 94,
    normalH: 94, // Normal height
    duckH: 60, // Height while ducking
    yv: 0,
    score: 0,
    hscore: 0,
    jump: 15,
  };

  const pbox = {
    x: player.x,
    y: 0,
    w: 80,
    h: 75,
  };

  const rngS = () => {
    gameStateRef.current.multiS = Math.floor(Math.random() * 3) + 1;
    gameStateRef.current.picS = 446 + Math.floor(Math.random() * 2) * 102;
  };

  const rngB = () => {
    gameStateRef.current.multiB = Math.floor(Math.random() * 3) + 1;
    gameStateRef.current.picB = 652 + Math.floor(Math.random() * 2) * 150;
  };

  const drawScores = (
    ctx: CanvasRenderingContext2D,
    canvas: HTMLCanvasElement,
    currentScore: number,
    highScore: number,
  ) => {
    // Save current context state
    ctx.save();

    // Set text properties
    ctx.font = '20px "Press Start 2P", monospace';
    ctx.fillStyle = "#535353";
    ctx.textAlign = "right";

    // Draw current score
    ctx.fillText(
      currentScore.toString().padStart(5, "0"),
      canvas.width - 25,
      30,
    );

    // Draw HI score
    ctx.fillText(
      "HI " + highScore.toString().padStart(5, "0"),
      canvas.width - 160,
      30,
    );

    // Restore context state
    ctx.restore();
  };

  const drawGameOverScreen = (
    ctx: CanvasRenderingContext2D,
    canvas: HTMLCanvasElement,
  ) => {
    // Draw scores using the final score
    drawScores(ctx, canvas, player.score, player.hscore);

    // Draw "GAME OVER" text
    ctx.save();
    ctx.font = '20px "Press Start 2P", monospace';
    ctx.fillStyle = "#535353";
    ctx.textAlign = "center";
    ctx.fillText("G A M E   O V E R", canvas.width / 2, canvas.height / 3);
    ctx.restore();

    // Draw restart button
    const buttonX = canvas.width / 2 - 36;
    const buttonY = canvas.height / 2 - 32;

    // Draw button with hover effect
    if (gameStateRef.current.isButtonHovered) {
      // Draw hover effect (slightly darker background)
      ctx.fillStyle = "rgba(83, 83, 83, 0.1)";
      ctx.fillRect(buttonX, buttonY, 72, 64);
    }

    ctx.drawImage(
      gameStateRef.current.sprImg,
      2,
      2,
      72,
      64,
      buttonX,
      buttonY,
      72,
      64,
    );
  };

  const gameover = () => {
    const state = gameStateRef.current;

    // Update high score if needed
    if (player.score > player.hscore) {
      player.hscore = player.score;
    }

    // Reset game speed and score
    state.gamespeed = 0;
    player.score = 0;

    // Reset player position and velocity
    player.y = 500;
    player.yv = 0;

    // Reset obstacles
    obsB.scroll = -200;
    obsS.scroll = -100;
    state.multiS = -1;
    state.multiB = -1;
    obsB.on = false;
    obsS.on = false;

    // Reset ground scrolling
    state.scoreInterval = 0;
    state.frameInterval = 0;
    state.groundscroll = 0;
    state.groundscroll2 = 0;
    state.tempstart = 0;
    state.groundbool = false;

    // Reset animation state
    state.frame = 1338; // Standing frame
    state.bool = false;
  };

  const handleKeyDown = (evt: KeyboardEvent) => {
    const state = gameStateRef.current;

    if (evt.key === "ArrowUp" || evt.key === " ") {
      // Up arrow or Space
      if (state.onG && !state.isJumping) {
        state.isJumping = true;
        player.yv = -player.jump;
      }
      if (state.gamespeed === 0) {
        state.gamespeed = 7;
      }
    }
    if (evt.key === "ArrowDown") {
      // Down arrow
      evt.preventDefault();
      if (!state.isDucking) {
        state.isDucking = true;
        if (state.onG) {
          player.h = player.duckH;
        }
      }
    }
  };

  const handleKeyUp = (evt: KeyboardEvent) => {
    const state = gameStateRef.current;

    if (evt.key === "ArrowDown") {
      // Down arrow
      evt.preventDefault();
      state.isDucking = false;
      player.h = player.normalH;
    }
  };

  const handleMouseMove = (evt: MouseEvent) => {
    const canvas = canvasRef.current;
    if (!canvas || gameStateRef.current.gamespeed !== 0) return;

    const rect = canvas.getBoundingClientRect();
    const x = evt.clientX - rect.left;
    const y = evt.clientY - rect.top;

    // Check if mouse is over restart button
    const buttonX = canvas.width / 2 - 36;
    const buttonY = canvas.height / 2 - 32;
    const isHovered =
      x >= buttonX && x <= buttonX + 72 && y >= buttonY && y <= buttonY + 64;

    if (isHovered !== gameStateRef.current.isButtonHovered) {
      gameStateRef.current.isButtonHovered = isHovered;
      // Force a redraw to update button appearance
      const ctx = canvas.getContext("2d");
      if (ctx) drawGameOverScreen(ctx, canvas);
    }
  };

  const handleClick = (evt: MouseEvent) => {
    const canvas = canvasRef.current;
    if (!canvas || gameStateRef.current.gamespeed !== 0) return;

    const rect = canvas.getBoundingClientRect();
    const x = evt.clientX - rect.left;
    const y = evt.clientY - rect.top;

    // Check if click is on restart button
    const buttonX = canvas.width / 2 - 36;
    const buttonY = canvas.height / 2 - 32;
    if (
      x >= buttonX &&
      x <= buttonX + 72 &&
      y >= buttonY &&
      y <= buttonY + 64
    ) {
      // Reset game state
      gameStateRef.current.gamespeed = 7;
      player.score = 0;
      player.y = 500; // Reset player position
      player.yv = 0; // Reset velocity

      // Reset obstacle positions
      obsB.scroll = -200;
      obsS.scroll = -100;
      gameStateRef.current.multiS = -1;
      gameStateRef.current.multiB = -1;
    }
  };

  const update = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const state = gameStateRef.current;
    const plat = {
      x: 0,
      y: canvas.height - 100,
      w: canvas.width,
      h: 5,
    };

    if (!state.onG) {
      player.yv += state.grav;
    } else {
      state.isJumping = false; // 땅에 닿으면 점프 상태 해제
    }

    player.y += player.yv;
    pbox.y = player.y;
    state.scoreInterval++;

    if (state.scoreInterval > 6 && state.gamespeed !== 0) {
      player.score++;
      state.scoreInterval = 0;
    }

    if (state.gamespeed < 17 && state.gamespeed !== 0) {
      // 속도 증가량을 2배로 증가 (score/100 -> score/50)
      state.gamespeed = 7 + player.score / 50;
    }

    state.onG = false;
    if (player.y + player.h > plat.y) {
      player.y = plat.y - player.h;
      state.onG = true;
    }

    // Collision detection
    if (
      pbox.x > canvas.width - obsB.scroll - player.w &&
      pbox.x < canvas.width - obsB.scroll + obsB.w * state.multiB &&
      pbox.y > obsB.y - pbox.h
    ) {
      gameover();
    }

    if (
      pbox.x > canvas.width - obsS.scroll - player.w &&
      pbox.x < canvas.width - obsS.scroll + obsS.w * state.multiS &&
      pbox.y > obsS.y - pbox.h
    ) {
      gameover();
    }

    // Animation frame update
    state.frameInterval++;
    if (state.frameInterval > 5) {
      state.bool = !state.bool;
      state.frameInterval = 0;
    }

    // Update frame based on state
    if (state.isDucking) {
      state.frame = state.bool ? 1866 : 1954; // Ducking animation frames
      player.h = player.duckH;
    } else if (state.onG) {
      state.frame = state.bool ? 1514 : 1602; // Running animation frames
      player.h = player.normalH;
    } else {
      state.frame = 1338; // Jumping frame
      player.h = player.normalH;
    }

    // Clear canvas
    ctx.fillStyle = "white";
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    // Ground scrolling
    state.groundscroll += state.gamespeed;
    ctx.drawImage(
      state.sprImg,
      0,
      104,
      2404,
      18,
      0 - state.groundscroll + state.tempstart,
      plat.y - 24,
      2404,
      18,
    );

    if (
      state.groundscroll - state.tempstart > 2404 - canvas.width ||
      state.groundbool
    ) {
      state.groundbool = true;
      state.groundscroll2 += state.gamespeed;
      ctx.drawImage(
        state.sprImg,
        0,
        104,
        canvas.width,
        18,
        0 - state.groundscroll2 + canvas.width,
        plat.y - 24,
        canvas.width,
        18,
      );

      if (
        state.groundscroll2 > canvas.width &&
        state.groundscroll - state.tempstart > 1000
      ) {
        state.tempstart = canvas.width;
        state.groundscroll = 20;
      }
      if (state.groundscroll2 > 2402) {
        state.groundscroll2 = 0;
        state.groundbool = false;
      }
    }

    // Draw character
    if (state.gamespeed !== 0) {
      ctx.drawImage(
        state.sprImg,
        state.frame,
        0,
        88,
        94,
        player.x,
        player.y,
        player.w,
        player.h,
      );
    } else {
      ctx.drawImage(
        state.sprImg,
        1338,
        0,
        88,
        94,
        player.x,
        player.y,
        player.w,
        player.h,
      );
    }

    // Draw obstacles
    if (!obsB.on) {
      obsS.on = true;
      if (state.multiS === -1) {
        rngS();
      }

      ctx.drawImage(
        state.sprImg,
        state.picS,
        2,
        obsS.w * state.multiS,
        obsS.h,
        canvas.width - obsS.scroll,
        obsS.y,
        obsS.w * state.multiS,
        obsS.h,
      );
      obsS.scroll += state.gamespeed;
      if (obsS.scroll > canvas.width + obsS.w * 3) {
        obsS.scroll = -100;
        state.multiS = -1;
        obsS.on = false;
      }
    }

    if (!obsS.on) {
      obsB.on = true;
      if (state.multiB === -1) {
        rngB();
      }

      ctx.drawImage(
        state.sprImg,
        652,
        2,
        obsB.w * state.multiB,
        obsB.h,
        canvas.width - obsB.scroll,
        obsB.y,
        obsB.w * state.multiB,
        obsB.h,
      );

      obsB.scroll += state.gamespeed;
      if (obsB.scroll > canvas.width + obsB.w * 3) {
        obsB.scroll = -200;
        state.multiB = -1;
        obsB.on = false;
      }
    }

    // Draw scores
    drawScores(ctx, canvas, player.score, player.hscore);

    // If game is over, draw game over screen
    if (state.gamespeed === 0) {
      drawGameOverScreen(ctx, canvas);
    }

    requestAnimationFrame(update);
  };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    // Load sprite image
    gameStateRef.current.sprImg.src = spriteImage;
    gameStateRef.current.sprImg.onload = () => {
      update();
    };

    // Add event listeners
    document.addEventListener("keydown", handleKeyDown);
    document.addEventListener("keyup", handleKeyUp);
    canvas.addEventListener("click", handleClick);
    canvas.addEventListener("mousemove", handleMouseMove);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      document.removeEventListener("keyup", handleKeyUp);
      canvas.removeEventListener("click", handleClick);
      canvas.removeEventListener("mousemove", handleMouseMove);
    };
  }, []);

  return (
    <div className="dino-game">
      <canvas
        ref={canvasRef}
        width={1000}
        height={400}
        className="dino-canvas"
      />
    </div>
  );
};

export default DinoGame;
