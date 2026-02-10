<script setup lang="ts">
import { useFetch } from '@vueuse/core';
import { computed } from 'vue';

interface StickerPub {
  id: string,
  set_id: string,
  // similarity: number
}

const url = computed(() => `/api/banned-stickers`)

const { data: stickers, error, execute: refetch } = useFetch(url, { refetch: true, updateDataOnError: true }).json<StickerPub[]>()

const unbanSticker = async (stickerId: string) => {
    const { data, error } = await useFetch(`/api/stickers/${stickerId}/unban`).post()
    console.log(data, error)
    if (error.value) {
      alert(error.value)
    }
}

</script>

<template>
  <div class="asdf">
    <!-- <h1>This is an ban similar view</h1>
     <v-slider
        v-model="maxSimilarity"
        thumb-label
        min="0.7"
        max="1"
        step="0.005"
      ></v-slider>
    {{ route.params.setId }}
    asdf
    {{ error }}

    <v-btn @click="toggleBan" v-if="isBanned" color="error">
      Unban
    </v-btn>
    <v-btn @click="toggleBan" v-else color="success">
      Ban
    </v-btn> -->


      <div v-if="stickers">
        <div v-for="sticker of stickers">
          <img :src="`/api/banned-sticker/${sticker.id}/thumbnail.png`" loading="lazy" width="128" height="128" />
          {{ sticker }}
          <v-btn @click="unbanSticker(sticker.id)">
            unban sticker
          </v-btn>
          <!-- TODO: both sticker and set ban views should recognize if the entity is already banned and offer to unban -->
          <v-btn :to="{name: 'banSimilarView', params: {setId: sticker.set_id, stickerId: sticker.id }}">
            ban similar view
          </v-btn>
        </div>
      </div>
  </div>
</template>

<style>

</style>
